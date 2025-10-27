# draco-gltf-rs

A small Rust utility to decode Draco-compressed mesh primitives embedded in glTF/GLB files using the
`KHR_draco_mesh_compression` extension. This crate exposes a convenience function `decode_draco` that
accepts a glTF `Primitive` and the associated buffers and returns a `DecodedPrimitive` with indices and
commonly-used vertex attributes (positions, normals, tangents, texcoords, colors, joints, weights).

## Features

- Decode Draco compressed primitives from glTF/GLB files
- Preserve common attribute semantics (POSITION, NORMAL, TEXCOORD_0, COLOR_0, JOINTS_0, WEIGHTS_0, ...)
- Return a convenient `DecodedPrimitive` struct with typed attribute arrays

## Adding to your project

This repository is not published on crates.io, add the dependency like:

```toml
[dependencies]
draco-gltf-rs = { git = "https://github.com/Niki123456123456/draco-gltf-rs.git" }
```

## Basic usage

The crate exposes `decode_draco` as the main entry point. Below is a minimal example that mirrors
the `examples/main.rs` included in the repository. It opens a GLB, imports buffers, finds a mesh's
primitive and decodes the Draco data into a `DecodedPrimitive`.

```rust
use draco_gltf_rs::{decode_draco, DecodedPrimitive};

fn decode_test_glb(path: &str) -> Result<DecodedPrimitive, Box<dyn std::error::Error>> {
	// Open the file
	let mut file = std::fs::File::open(path)?;

	// Parse the GLB without validation (imports blob and document)
	let glb = gltf::Gltf::from_reader_without_validation(&mut file)?;
	let doc = glb.document;
	let blob = glb.blob;

	// Load buffers referenced by the document
	let buffer_data = gltf::import_buffers(&doc, None, blob)?;

	// Choose a mesh and primitive (example takes the last ones)
	let mesh = doc.meshes().last().ok_or("No meshes found in GLB")?;
	let prim = mesh.primitives().last().ok_or("No primitives found in mesh")?;

	// Decode Draco-compressed data (returns DecodedPrimitive)
	let decoded = decode_draco(&prim, &doc, &buffer_data)?;

	Ok(decoded)
}

fn main() {
	// Replace with your GLB path
	let decoded = decode_test_glb("examples/test.glb").expect("decode failed");

	// Example: access positions and indices
	if let Some(positions) = decoded.positions {
		println!("Loaded {} vertex positions", positions.len());
	}
	println!("Loaded {} indices", decoded.indices.len());
}
```


## Notes

- Only primitives using `KHR_draco_mesh_compression` and TRIANGLES mode are supported.
- The crate relies on `draco_decoder` to perform the actual Draco decoding; see `Cargo.toml` for the
  referenced dependency.

Contributions, bug reports and PRs are welcome.

