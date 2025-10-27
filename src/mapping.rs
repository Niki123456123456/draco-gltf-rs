pub fn dracokey_to_semantic(key: &str) -> Option<gltf::Semantic> {
    if key == "POSITION" {
        return Some(gltf::Semantic::Positions);
    }
    if key == "NORMAL" {
        return Some(gltf::Semantic::Normals);
    }
    if key == "TANGENT" {
        return Some(gltf::Semantic::Tangents);
    }

    let split = key.split_once('_')?;
    let (kind, idx_s) = split;
    let idx: u32 = idx_s.parse().ok()?;
    match kind {
        "TEXCOORD" => Some(gltf::Semantic::TexCoords(idx)),
        "COLOR" => Some(gltf::Semantic::Colors(idx)),
        "JOINTS" => Some(gltf::Semantic::Joints(idx)),
        "WEIGHTS" => Some(gltf::Semantic::Weights(idx)),
        _ => None,
    }
}

pub fn map_draco_dt(dt_u8: u8) -> draco_decoder::AttributeDataType {
    match dt_u8 {
        // these match draco::DataType enum discriminants used by the lib
        1 => draco_decoder::AttributeDataType::Int8,
        2 => draco_decoder::AttributeDataType::UInt8,
        3 => draco_decoder::AttributeDataType::Int16,
        4 => draco_decoder::AttributeDataType::UInt16,
        5 => draco_decoder::AttributeDataType::Int32,
        6 => draco_decoder::AttributeDataType::UInt32,
        7 => draco_decoder::AttributeDataType::Float32,
        // draco has 64-bit and float64 but glTF vertex streams won’t use them
        _ => draco_decoder::AttributeDataType::Float32,
    }
}

pub fn comp_size_bytes(ct: gltf::accessor::DataType) -> usize {
    use gltf::accessor::DataType::*;
    match ct {
        I8 | U8 => 1,
        I16 | U16 => 2,
        U32 | F32 => 4,
        // I32 isn't allowed in glTF 2.0 accessors; F64 not used here.
        _ => 4,
    }
}

pub fn dims_count(d: gltf::accessor::Dimensions) -> usize {
    use gltf::accessor::Dimensions::*;
    match d {
        Scalar => 1,
        Vec2 => 2,
        Vec3 => 3,
        Vec4 => 4,
        // tangents must be Vec4 in glTF; matrices not used for vertex streams here.
        _ => 4,
    }
}

pub fn as_f32n<const N: usize>(bytes: &[u8]) -> Vec<[f32; N]> {
    bytes
        .chunks_exact(4 * N)
        .map(|c| {
            let mut v = [0f32; N];
            for i in 0..N {
                let base = i * 4;
                v[i] = f32::from_le_bytes([c[base], c[base + 1], c[base + 2], c[base + 3]]);
            }
            v
        })
        .collect()
}
pub fn as_u16x4(bytes: &[u8]) -> Vec<[u16; 4]> {
    bytes
        .chunks_exact(8)
        .map(|c| {
            [
                u16::from_le_bytes([c[0], c[1]]),
                u16::from_le_bytes([c[2], c[3]]),
                u16::from_le_bytes([c[4], c[5]]),
                u16::from_le_bytes([c[6], c[7]]),
            ]
        })
        .collect()
}
pub fn as_u8x4(bytes: &[u8]) -> Vec<[u8; 4]> {
    bytes
        .chunks_exact(4)
        .map(|c| [c[0], c[1], c[2], c[3]])
        .collect()
}