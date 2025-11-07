#[derive(Debug, Clone, Default)]
pub struct DecodedPrimitive {
    pub indices: Vec<u32>,
    pub positions: Option<Vec<[f32; 3]>>,
    pub normals: Option<Vec<[f32; 3]>>,
    pub tangents: Option<Vec<[f32; 4]>>,
    pub texcoords: std::collections::HashMap<u32, Vec<[f32; 2]>>,
    pub colors: std::collections::HashMap<u32, Vec<[f32; 4]>>,
    pub joints: std::collections::HashMap<u32, Vec<[u16; 4]>>,
    pub weights: std::collections::HashMap<u32, Vec<[f32; 4]>>,
}

#[derive(Debug, thiserror::Error)]
pub enum DracoLoadError {
    #[error("primitive doesn't use KHR_draco_mesh_compression")]
    NotDraco,
    #[error("missing or malformed KHR_draco_mesh_compression extension")]
    BadExtension,
    #[error("bufferView {0} not found")]
    BadBufferView(usize),
    #[error("buffer index {0} not found")]
    BadBuffer(usize),
    #[error("attribute mapping missing POSITION accessor (needed for vertex count)")]
    NoPositionAccessor,
    #[error("indices accessor missing for TRIANGLES primitive")]
    NoIndicesAccessor,
    #[error("draco decode failed")]
    DracoDecode,
    #[error("attribute id {0} from Draco stream not in glTF extension attributes map")]
    UnknownAttributeId(u32),
    #[error("unsupported primitive mode (only TRIANGLES supported)")]
    UnsupportedMode,
}

#[derive(serde::Deserialize)]
struct DracoExt {
    #[serde(rename = "bufferView")]
    buffer_view: usize,
    attributes: std::collections::HashMap<String, u32>, // semantic -> draco unique id
}

struct AttrSlice<'a> {
    unique_id: u32,
    bytes: &'a [u8],
    dim: usize,
    dt: draco_decoder::AttributeDataType,
}

pub struct AttrInfo {
    pub unique_id: u32, // Draco attribute unique id
    pub dim: u32,       // number of components, e.g. 3 for POSITION
    pub data_type: u8,  // draco::DataType as a small integer
}

mod mapping;
use mapping::*;

pub async fn decode_draco(
    p: &gltf::mesh::Primitive<'_>,
    document: &gltf::Document,
    buffers: &Vec<gltf::buffer::Data>,
    infos: &Vec<AttrInfo>,
) -> Result<DecodedPrimitive, DracoLoadError> {
    let (draco_bytes, cfg, index_comp, index_count, vertex_count, draco_ext) =
        prozes_in(p, document, buffers, infos)?;
    let raw = draco_decoder::decode_mesh(draco_bytes, &cfg).await.ok_or(DracoLoadError::DracoDecode)?;
    return prozes_out(
        &raw,
        index_comp,
        index_count,
        vertex_count,
        infos,
        p,
        draco_ext,
    );
}

fn prozes_in<'a>(
    p: &'a gltf::mesh::Primitive<'_>,
    document: &'a gltf::Document,
    buffers: &'a Vec<gltf::buffer::Data>,
    infos: &'a Vec<AttrInfo>,
) -> Result<
    (
        &'a [u8],
        draco_decoder::MeshDecodeConfig,
        gltf::accessor::DataType,
        usize,
        usize,
        DracoExt,
    ),
    DracoLoadError,
> {
    if p.mode() != gltf::mesh::Mode::Triangles {
        return Err(DracoLoadError::UnsupportedMode);
    }
    let value = p
        .extension_value("KHR_draco_mesh_compression")
        .ok_or(DracoLoadError::NotDraco)?;
    let draco_ext: DracoExt =
        serde_json::from_value(value.clone()).map_err(|_| DracoLoadError::BadExtension)?;

    let draco_bytes: &[u8] = get_buffer(document, buffers, draco_ext.buffer_view)?;

    let vertex_count = p
        .get(&gltf::Semantic::Positions)
        .ok_or(DracoLoadError::NoPositionAccessor)?
        .count();

    let indices_accessor = p.indices().ok_or(DracoLoadError::NoIndicesAccessor)?;
    let index_count: usize = indices_accessor.count();
    let mut index_comp: gltf::accessor::DataType = indices_accessor.data_type();
    if index_comp == gltf::accessor::DataType::U8 {
        // workaround because draco_decoder has not yet logic for u8
        index_comp = gltf::accessor::DataType::U16;
    }

    let mut cfg: draco_decoder::MeshDecodeConfig =
        draco_decoder::MeshDecodeConfig::new(vertex_count as u32, index_count as u32);
    for info in infos {
        cfg.add_attribute(info.dim, map_draco_dt(info.data_type));
    }
    return Ok((
        draco_bytes,
        cfg,
        index_comp,
        index_count,
        vertex_count,
        draco_ext,
    ));
}

fn prozes_out(
    raw: &[u8],
    index_comp: gltf::accessor::DataType,
    index_count: usize,
    vertex_count: usize,
    infos: &Vec<AttrInfo>,
    p: &gltf::mesh::Primitive<'_>,
    draco_ext: DracoExt,
) -> Result<DecodedPrimitive, DracoLoadError> {
    let index_bytes: usize = index_count * comp_size_bytes(index_comp);
    let indices = get_indices(&raw, index_bytes, index_comp)?;

    let mut cursor = index_bytes;
    let mut attr_blocks: Vec<AttrSlice<'_>> = Vec::with_capacity(infos.len());
    for info in infos {
        let elem_size = match info.data_type {
            1 | 2 => 1,     // i8/u8
            3 | 4 => 2,     // i16/u16
            5 | 6 | 7 => 4, // i32/u32/f32
            _ => 4,
        };
        let byte_len = vertex_count * (info.dim as usize) * elem_size;
        let blk = &raw[cursor..cursor + byte_len];
        cursor += byte_len;
        attr_blocks.push(AttrSlice {
            unique_id: info.unique_id,
            bytes: blk,
            dim: info.dim as usize,
            dt: map_draco_dt(info.data_type),
        });
    }

    let mut dracoid_to_sem: std::collections::HashMap<u32, (gltf::Semantic, gltf::Accessor)> =
        std::collections::HashMap::new();
    for (k, id) in &draco_ext.attributes {
        if let Some(sem) = dracokey_to_semantic(k) {
            if let Some(acc) = p.get(&sem) {
                dracoid_to_sem.insert(*id, (sem, acc));
            }
        }
    }

    let mut out = DecodedPrimitive {
        indices,
        ..Default::default()
    };

    fill_primitive(&mut out, &attr_blocks, &dracoid_to_sem)?;
    return Ok(out);
}

fn get_buffer<'a>(
    document: &'a gltf::Document,
    buffers: &'a Vec<gltf::buffer::Data>,
    index: usize,
) -> Result<&'a [u8], DracoLoadError> {
    let bv = document
        .views()
        .find(|v| v.index() == index)
        .ok_or(DracoLoadError::BadBufferView(index))?;

    let buffer_idx = bv.buffer().index();
    let buf = buffers
        .get(buffer_idx)
        .ok_or(DracoLoadError::BadBuffer(buffer_idx))?;

    let start = bv.offset();
    let end = start + bv.length();
    Ok(&buf[start..end])
}

fn get_indices(
    raw: &[u8],
    index_bytes: usize,
    index_comp: gltf::accessor::DataType,
) -> Result<Vec<u32>, DracoLoadError> {
    let indices_bytes = &raw[0..index_bytes];
    let indices: Vec<u32> = match index_comp {
        gltf::accessor::DataType::U16 => indices_bytes
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]) as u32)
            .collect(),
        gltf::accessor::DataType::U32 => indices_bytes
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect(),
        gltf::accessor::DataType::U8 => indices_bytes.iter().map(|&b| b as u32).collect(),
        _ => return Err(DracoLoadError::DracoDecode),
    };
    return Ok(indices);
}

fn fill_primitive(
    p: &mut DecodedPrimitive,
    attr_blocks: &Vec<AttrSlice<'_>>,
    dracoid_to_sem: &std::collections::HashMap<u32, (gltf::Semantic, gltf::Accessor)>,
) -> Result<(), DracoLoadError> {
    for blk in attr_blocks {
        let (sem, acc) = dracoid_to_sem
            .get(&blk.unique_id)
            .ok_or(DracoLoadError::UnknownAttributeId(blk.unique_id))?;

        let acc_dims = dims_count(acc.dimensions());
        debug_assert_eq!(acc_dims, blk.dim, "Draco dim != accessor dim");

        match *sem {
            gltf::Semantic::Positions => {
                p.positions = Some(as_f32n::<3>(blk.bytes));
            }
            gltf::Semantic::Normals => {
                p.normals = Some(as_f32n::<3>(blk.bytes));
            }
            gltf::Semantic::Tangents => {
                p.tangents = Some(as_f32n::<4>(blk.bytes));
            }
            gltf::Semantic::TexCoords(set) => {
                // In practice Draco provides TEXCOORD as f32; if U16/U8 normalized were used
                // you could map via acc.normalized() to convert to f32 in your renderer.
                p.texcoords.insert(set, as_f32n::<2>(blk.bytes));
            }
            gltf::Semantic::Colors(set) => {
                // Could be f32 or normalized U8. Handle common f32 path here.
                if matches!(blk.dt, draco_decoder::AttributeDataType::Float32) {
                    p.colors.insert(set, as_f32n::<4>(blk.bytes));
                } else {
                    // fall back: keep as normalized 8-bit expanded to f32 [0..1]
                    let raw = as_u8x4(blk.bytes);
                    let conv = raw
                        .into_iter()
                        .map(|c| {
                            [
                                c[0] as f32 / 255.0,
                                c[1] as f32 / 255.0,
                                c[2] as f32 / 255.0,
                                c[3] as f32 / 255.0,
                            ]
                        })
                        .collect();
                    p.colors.insert(set, conv);
                }
            }
            gltf::Semantic::Joints(set) => {
                // Often u8 or u16; we store u16
                if matches!(blk.dt, draco_decoder::AttributeDataType::UInt16) {
                    p.joints.insert(set, as_u16x4(blk.bytes));
                } else {
                    // widen u8->u16
                    let v: Vec<[u16; 4]> = blk
                        .bytes
                        .chunks_exact(4)
                        .map(|c| [c[0] as u16, c[1] as u16, c[2] as u16, c[3] as u16])
                        .collect();
                    p.joints.insert(set, v);
                }
            }
            gltf::Semantic::Weights(set) => {
                // Usually f32; if normalized u8/u16 were used, convert to f32.
                if matches!(blk.dt, draco_decoder::AttributeDataType::Float32) {
                    p.weights.insert(set, as_f32n::<4>(blk.bytes));
                } else if matches!(blk.dt, draco_decoder::AttributeDataType::UInt16) {
                    let v: Vec<[f32; 4]> = blk
                        .bytes
                        .chunks_exact(8)
                        .map(|c| {
                            [
                                u16::from_le_bytes([c[0], c[1]]) as f32 / 65535.0,
                                u16::from_le_bytes([c[2], c[3]]) as f32 / 65535.0,
                                u16::from_le_bytes([c[4], c[5]]) as f32 / 65535.0,
                                u16::from_le_bytes([c[6], c[7]]) as f32 / 65535.0,
                            ]
                        })
                        .collect();
                    p.weights.insert(set, v);
                } else {
                    let v: Vec<[f32; 4]> = blk
                        .bytes
                        .chunks_exact(4)
                        .map(|c| {
                            [
                                c[0] as f32 / 255.0,
                                c[1] as f32 / 255.0,
                                c[2] as f32 / 255.0,
                                c[3] as f32 / 255.0,
                            ]
                        })
                        .collect();
                    p.weights.insert(set, v);
                }
            }
        }
    }
    return Ok(());
}

#[cfg(test)]
mod tests {
    use super::*;

     #[tokio::test]
    async  fn test_decode_test_glb() -> Result<(), Box<dyn std::error::Error>> {
        let path = "examples/test.glb";
        let primitive = decode_test_glb(path).await?;

        // Validate positions
        let positions = primitive.positions.ok_or("Missing positions attribute")?;
        assert!(!positions.is_empty(), "Positions should not be empty");

        // Validate texcoords (using channel 0)
        let texcoords = primitive
            .texcoords
            .get(&0)
            .ok_or("Missing texcoords[0] attribute")?;
        assert!(!texcoords.is_empty(), "Texcoords should not be empty");

        // Validate indices
        assert!(!primitive.indices.is_empty(), "Indices should not be empty");

        Ok(())
    }

    pub async  fn decode_test_glb(path: &str) -> Result<DecodedPrimitive, Box<dyn std::error::Error>> {
        // Open the file safely
        let mut file = std::fs::File::open(path)?;

        // Read the glTF binary without validation
        let glb = gltf::Gltf::from_reader_without_validation(&mut file)?;
        let doc = glb.document;
        let blob = glb.blob;

        // Import all referenced buffers
        let buffer_data = gltf::import_buffers(&doc, None, blob)?;

        // Get the last mesh and primitive
        let mesh = doc.meshes().last().ok_or("No meshes found in GLB")?;
        let prim = mesh
            .primitives()
            .last()
            .ok_or("No primitives found in mesh")?;

        // Decode Draco data
        let decoded = decode_draco(&prim, &doc, &buffer_data, &vec![AttrInfo {
            unique_id: 0,
            dim: 3,
            data_type: 9,
        }, AttrInfo {
            unique_id: 1,
            dim: 2,
            data_type: 9,
        }],).await?;

        Ok(decoded)
    }
}
