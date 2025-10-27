fn main() {
    decode_test_glb("examples/test.glb").unwrap();
}

pub fn decode_test_glb(
    path: &str,
) -> Result<draco_gltf_rs::DecodedPrimitive, Box<dyn std::error::Error>> {
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
    let decoded = draco_gltf_rs::decode_draco(&prim, &doc, &buffer_data)?;

    Ok(decoded)
}
