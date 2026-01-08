@fragment
fn frag_main(@builtin(position) pos : vec4f) -> @location(0) u32 {
    let dist: f32 = length((pos.xy - 128) / 128.0);
    return u32(clamp(dist, 0.0, 1.0) * 0xFF);
}
