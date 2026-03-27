use asteroids_3d::run;

// Credit for the majority of wgpu implementation: https://sotrh.github.io/learn-wgpu/
//      wgpu implementation follows the guide up to but not including the hdr portion
// Credit for the Spaceship model https://sketchfab.com/3d-models/light-fighter-spaceship-free-51616ef53af84fe595c5603cd3e0f3e1
// TODO get a model of an asteroid
// TODO Instant crate is unmaintained, they recommend a different crate https://crates.io/crates/instant
// TODO move stuff to their own files
fn main() {
    let _ = run().unwrap();
}