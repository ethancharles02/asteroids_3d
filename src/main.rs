use asteroids_3d::run;

// "Asteroid low poly" (https://skfb.ly/oz7ZN) by pasquill is licensed under Creative Commons Attribution (http://creativecommons.org/licenses/by/4.0/).
// Credit for the majority of wgpu implementation: https://sotrh.github.io/learn-wgpu/
//      wgpu implementation follows the guide up to but not including the hdr portion
// Credit for the Spaceship model https://sketchfab.com/3d-models/light-fighter-spaceship-free-51616ef53af84fe595c5603cd3e0f3e1
// TODO clean up warnings
fn main() {
    let _ = run().unwrap();
}