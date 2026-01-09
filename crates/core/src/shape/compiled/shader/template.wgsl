struct ShaderArgument {
    offset: u32,
    length: u32,
}

struct TileBounds {
    min_lat_deg: i32,  // southwest corner
    min_lon_deg: i32,
    lat_span_deg: i32, // tile height in degrees
    lon_span_deg: i32, // tile width in degrees
}

@group(0) @binding(0)
var<storage, read> arguments: array<ShaderArgument>;

@group(0) @binding(1)
var<storage, read> argument_data: array<u32>;

@group(0) @binding(2)
var<uniform> tile_bounds: TileBounds;

override USE_ELLIPSOID: bool = true;
override USE_HIGH_PRECISION: bool = false;

const COORD_SCALE: i32 = 10000000; // lat/lon stored as degrees * 1e7
const WGS84_A: f32 = 6378137.0;
const WGS84_B: f32 = 6356752.314245;
const WGS84_F: f32 = 1.0 / 298.257223563;
const EARTH_RADIUS: f32 = 6371000.0;
const DEG_TO_RAD: f32 = 0.017453292519943295;

// const WGS84_A_F64: f64 = 6378137.0;
// const WGS84_B_F64: f64 = 6356752.314245;
// const WGS84_F_F64: f64 = 1.0 / 298.257223563;
// const EARTH_RADIUS_F64: f64 = 6371000.0;
// const DEG_TO_RAD_F64: f64 = 0.017453292519943295;

fn popArgument(idx_ptr: ptr<function, u32>) -> ShaderArgument {
    let idx = *idx_ptr;
    *idx_ptr = idx + 1u;
    return arguments[idx];
}

fn argument_read_u32(argument: ShaderArgument, index: u32) -> u32 {
    return argument_data[argument.offset + index];
}

fn argument_read_i32(argument: ShaderArgument, index: u32) -> i32 {
    return bitcast<i32>(argument_data[argument.offset + index]);
}

fn argument_read_f32(argument: ShaderArgument, index: u32) -> f32 {
    return bitcast<f32>(argument_data[argument.offset + index]);
}

// ============================================================================
// f32 implementations
// ============================================================================

// Haversine formula for spherical earth
fn haversine_distance(lat1: f32, lon1: f32, lat2: f32, lon2: f32) -> f32 {
    let phi1 = lat1 * DEG_TO_RAD;
    let phi2 = lat2 * DEG_TO_RAD;
    let dphi = (lat2 - lat1) * DEG_TO_RAD;
    let dlambda = (lon2 - lon1) * DEG_TO_RAD;
    
    let a = sin(dphi * 0.5) * sin(dphi * 0.5) +
            cos(phi1) * cos(phi2) *
            sin(dlambda * 0.5) * sin(dlambda * 0.5);
    let c = 2.0 * atan2(sqrt(a), sqrt(1.0 - a));
    
    return EARTH_RADIUS * c;
}

// Vincenty's inverse formula for ellipsoidal earth
fn vincenty_distance(lat1: f32, lon1: f32, lat2: f32, lon2: f32) -> f32 {
    let phi1 = lat1 * DEG_TO_RAD;
    let phi2 = lat2 * DEG_TO_RAD;
    let L = (lon2 - lon1) * DEG_TO_RAD;
    
    let U1 = atan((1.0 - WGS84_F) * tan(phi1));
    let U2 = atan((1.0 - WGS84_F) * tan(phi2));
    let sinU1 = sin(U1);
    let cosU1 = cos(U1);
    let sinU2 = sin(U2);
    let cosU2 = cos(U2);
    
    var lambda = L;
    var lambda_prev: f32;
    var iter = 0u;
    
    var sinLambda: f32;
    var cosLambda: f32;
    var sinSigma: f32;
    var cosSigma: f32;
    var sigma: f32;
    var sinAlpha: f32;
    var cos2Alpha: f32;
    var cos2SigmaM: f32;
    var C: f32;
    
    loop {
        if (iter >= 100u) { break; }
        
        sinLambda = sin(lambda);
        cosLambda = cos(lambda);
        sinSigma = sqrt((cosU2 * sinLambda) * (cosU2 * sinLambda) +
                        (cosU1 * sinU2 - sinU1 * cosU2 * cosLambda) *
                        (cosU1 * sinU2 - sinU1 * cosU2 * cosLambda));
        
        if (sinSigma == 0.0) { return 0.0; }
        
        cosSigma = sinU1 * sinU2 + cosU1 * cosU2 * cosLambda;
        sigma = atan2(sinSigma, cosSigma);
        sinAlpha = cosU1 * cosU2 * sinLambda / sinSigma;
        cos2Alpha = 1.0 - sinAlpha * sinAlpha;

        if (cos2Alpha < 1e-10) {
            cos2SigmaM = 0.0;
        } else {
            cos2SigmaM = cosSigma - 2.0 * sinU1 * sinU2 / cos2Alpha;
        }
        
        // isNan
        if (cos2SigmaM != cos2SigmaM) { cos2SigmaM = 0.0; }
        
        C = WGS84_F / 16.0 * cos2Alpha * (4.0 + WGS84_F * (4.0 - 3.0 * cos2Alpha));
        lambda_prev = lambda;
        lambda = L + (1.0 - C) * WGS84_F * sinAlpha *
                 (sigma + C * sinSigma * (cos2SigmaM + C * cosSigma *
                  (-1.0 + 2.0 * cos2SigmaM * cos2SigmaM)));

        if (abs(lambda - lambda_prev) < 1e-6) { break; }
        iter += 1u;
    }
    
    let u2 = cos2Alpha * (WGS84_A * WGS84_A - WGS84_B * WGS84_B) / (WGS84_B * WGS84_B);
    let A = 1.0 + u2 / 16384.0 * (4096.0 + u2 * (-768.0 + u2 * (320.0 - 175.0 * u2)));
    let B = u2 / 1024.0 * (256.0 + u2 * (-128.0 + u2 * (74.0 - 47.0 * u2)));
    let deltaSigma = B * sinSigma * (cos2SigmaM + B / 4.0 *
                     (cosSigma * (-1.0 + 2.0 * cos2SigmaM * cos2SigmaM) -
                      B / 6.0 * cos2SigmaM * (-3.0 + 4.0 * sinSigma * sinSigma) *
                      (-3.0 + 4.0 * cos2SigmaM * cos2SigmaM)));
    
    return WGS84_B * A * (sigma - deltaSigma);
}

// ============================================================================
// f64 implementations
// ============================================================================

// Haversine formula for spherical earth (f64)
// fn haversine_distance_f64(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
//     let phi1 = lat1 * DEG_TO_RAD_F64;
//     let phi2 = lat2 * DEG_TO_RAD_F64;
//     let dphi = (lat2 - lat1) * DEG_TO_RAD_F64;
//     let dlambda = (lon2 - lon1) * DEG_TO_RAD_F64;
    
//     let a = sin(dphi * 0.5) * sin(dphi * 0.5) +
//             cos(phi1) * cos(phi2) *
//             sin(dlambda * 0.5) * sin(dlambda * 0.5);
//     let c = 2.0 * atan2(sqrt(a), sqrt(1.0 - a));
    
//     return EARTH_RADIUS_F64 * c;
// }

// // Vincenty's inverse formula for ellipsoidal earth (f64)
// fn vincenty_distance_f64(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
//     let phi1 = lat1 * DEG_TO_RAD_F64;
//     let phi2 = lat2 * DEG_TO_RAD_F64;
//     let L = (lon2 - lon1) * DEG_TO_RAD_F64;
    
//     let U1 = atan((1.0 - WGS84_F_F64) * tan(phi1));
//     let U2 = atan((1.0 - WGS84_F_F64) * tan(phi2));
//     let sinU1 = sin(U1);
//     let cosU1 = cos(U1);
//     let sinU2 = sin(U2);
//     let cosU2 = cos(U2);
    
//     var lambda = L;
//     var lambda_prev: f64;
//     var iter = 0u;
    
//     var sinLambda: f64;
//     var cosLambda: f64;
//     var sinSigma: f64;
//     var cosSigma: f64;
//     var sigma: f64;
//     var sinAlpha: f64;
//     var cos2Alpha: f64;
//     var cos2SigmaM: f64;
//     var C: f64;
    
//     loop {
//         if (iter >= 100u) { break; }
        
//         sinLambda = sin(lambda);
//         cosLambda = cos(lambda);
//         sinSigma = sqrt((cosU2 * sinLambda) * (cosU2 * sinLambda) +
//                         (cosU1 * sinU2 - sinU1 * cosU2 * cosLambda) *
//                         (cosU1 * sinU2 - sinU1 * cosU2 * cosLambda));
        
//         if (sinSigma == 0.0) { return f64(0.0); }
        
//         cosSigma = sinU1 * sinU2 + cosU1 * cosU2 * cosLambda;
//         sigma = atan2(sinSigma, cosSigma);
//         sinAlpha = cosU1 * cosU2 * sinLambda / sinSigma;
//         cos2Alpha = 1.0 - sinAlpha * sinAlpha;
//         cos2SigmaM = cosSigma - 2.0 * sinU1 * sinU2 / cos2Alpha;
        
//         // isNan
//         if (cos2SigmaM != cos2SigmaM) { cos2SigmaM = 0.0; }
        
//         C = WGS84_F_F64 / 16.0 * cos2Alpha * (4.0 + WGS84_F_F64 * (4.0 - 3.0 * cos2Alpha));
//         lambda_prev = lambda;
//         lambda = L + (1.0 - C) * WGS84_F_F64 * sinAlpha *
//                  (sigma + C * sinSigma * (cos2SigmaM + C * cosSigma *
//                   (-1.0 + 2.0 * cos2SigmaM * cos2SigmaM)));
        
//         if (abs(lambda - lambda_prev) < 1e-12) { break; }
//         iter += 1u;
//     }
    
//     let u2 = cos2Alpha * (WGS84_A_F64 * WGS84_A_F64 - WGS84_B_F64 * WGS84_B_F64) / (WGS84_B_F64 * WGS84_B_F64);
//     let A = 1.0 + u2 / 16384.0 * (4096.0 + u2 * (-768.0 + u2 * (320.0 - 175.0 * u2)));
//     let B = u2 / 1024.0 * (256.0 + u2 * (-128.0 + u2 * (74.0 - 47.0 * u2)));
//     let deltaSigma = B * sinSigma * (cos2SigmaM + B / 4.0 *
//                      (cosSigma * (-1.0 + 2.0 * cos2SigmaM * cos2SigmaM) -
//                       B / 6.0 * cos2SigmaM * (-3.0 + 4.0 * sinSigma * sinSigma) *
//                       (-3.0 + 4.0 * cos2SigmaM * cos2SigmaM)));
    
//     return WGS84_B_F64 * A * (sigma - deltaSigma);
// }

// Instruction: Point
fn point(sample: vec2<f32>, idx_ptr: ptr<function, u32>) -> i32 {
    let argument = popArgument(idx_ptr);
    let x = argument_read_i32(argument, 0u);
    let y = argument_read_i32(argument, 1u);
    
    var distance_m: f32;

    if (USE_HIGH_PRECISION) {
        // Compute sample coords in scaled space, then divide once
        // let sample_lon_scaled = f64(tile_bounds.min_lon_deg) + f64(sample.x) * f64(tile_bounds.lon_span_deg);
        // let sample_lat_scaled = f64(tile_bounds.min_lat_deg) + f64(sample.y) * f64(tile_bounds.lat_span_deg);

        // if (USE_ELLIPSOID) {
        //     distance_m = f32(vincenty_distance_f64(
        //         sample_lat_scaled / f64(COORD_SCALE), 
        //         sample_lon_scaled / f64(COORD_SCALE),
        //         f64(y) / f64(COORD_SCALE), 
        //         f64(x) / f64(COORD_SCALE)
        //     ));
        // } else {
        //     distance_m = f32(haversine_distance_f64(
        //         sample_lat_scaled / f64(COORD_SCALE), 
        //         sample_lon_scaled / f64(COORD_SCALE),
        //         f64(y) / f64(COORD_SCALE), 
        //         f64(x) / f64(COORD_SCALE)
        //     ));
        // }
        distance_m = 0;
    } else {
        // f32 path
        let sample_lon_scaled = f32(tile_bounds.min_lon_deg) + sample.x * f32(tile_bounds.lon_span_deg);
        let sample_lat_scaled = f32(tile_bounds.min_lat_deg + tile_bounds.lat_span_deg) - sample.y * f32(tile_bounds.lat_span_deg);

        if (USE_ELLIPSOID) {
            distance_m = vincenty_distance(
                sample_lat_scaled / f32(COORD_SCALE),
                sample_lon_scaled / f32(COORD_SCALE),
                f32(y) / f32(COORD_SCALE),
                f32(x) / f32(COORD_SCALE)
            );
        } else {
            distance_m = haversine_distance(
                sample_lat_scaled / f32(COORD_SCALE),
                sample_lon_scaled / f32(COORD_SCALE),
                f32(y) / f32(COORD_SCALE),
                f32(x) / f32(COORD_SCALE)
            );
        }
    }
    
    return i32(distance_m * 100.0);
}

fn compute(sample: vec2<f32>, idx_ptr: ptr<function, u32>) -> i32 {
    // filled in by code generator
}

@fragment
fn main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4f {
    // frag_coord is in pixel coordinates [0, 256)
    // Convert to [0,1] tile-local coordinates
    let sample = frag_coord.xy / 256.0;
    
    // Call point with our sample
    var arg_idx = 0u;
    let distance_cm_i32 = compute(sample, &arg_idx);
    let distance_cm = bitcast<u32>(distance_cm_i32);
    
    return vec4f(
        f32((distance_cm & 0x000000FF) >> 0) / 255.0,
        f32((distance_cm & 0x0000FF00) >> 8) / 255.0,
        f32((distance_cm & 0x00FF0000) >> 16) / 255.0,
        // f32((distance_cm & 0xFF000000) >> 24) / 255.0,
        1.0
    );
}
