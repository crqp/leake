struct Params {
    width: u32,
    height: u32,
    blocks_x: u32,
    blocks_y: u32,
    kind: u32,
    dither: u32,
    refine: u32,
    red_bits: u32,
    green_bits: u32,
    blue_bits: u32,
    alpha_bits: u32,
    opaque: u32,
    true_colour: u32,
    packed_dithers: u32,
    reserved0: u32,
    reserved1: u32,
}

@group(0) @binding(0) var<storage, read> source: array<u32>;
@group(0) @binding(1) var<storage, read_write> destination: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var<storage, read> dither_matrix: array<u32>;
@group(0) @binding(4) var<storage, read> shades: array<u32>;

struct ColourBlock {
    c0: u32,
    c1: u32,
    indices: u32,
    error: u32,
}

struct AlphaBlock {
    a0: u32,
    a1: u32,
    low: u32,
    high: u32,
    error: u32,
}

fn red(pixel: u32) -> u32 {
    return pixel & 0xffu;
}

fn green(pixel: u32) -> u32 {
    return (pixel >> 8u) & 0xffu;
}

fn blue(pixel: u32) -> u32 {
    return (pixel >> 16u) & 0xffu;
}

fn alpha(pixel: u32) -> u32 {
    return (pixel >> 24u) & 0xffu;
}

fn to_565(r: u32, g: u32, b: u32) -> u32 {
    return ((r >> 3u) << 11u) | ((g >> 2u) << 5u) | (b >> 3u);
}

fn expand_565(code: u32) -> vec3<u32> {
    let r = (code >> 11u) & 0x1fu;
    let g = (code >> 5u) & 0x3fu;
    let b = code & 0x1fu;
    return vec3<u32>((r << 3u) | (r >> 2u), (g << 2u) | (g >> 4u), (b << 3u) | (b >> 2u));
}

fn shift_code(code: u32, dimension: u32, delta: i32) -> u32 {
    var r = (code >> 11u) & 0x1fu;
    var g = (code >> 5u) & 0x3fu;
    var b = code & 0x1fu;
    if (dimension == 0u) {
        r = u32(clamp(i32(r) + delta, 0, 31));
    } else if (dimension == 1u) {
        g = u32(clamp(i32(g) + delta, 0, 63));
    } else {
        b = u32(clamp(i32(b) + delta, 0, 31));
    }
    return (r << 11u) | (g << 5u) | b;
}

fn distance(left: vec3<u32>, right: vec3<u32>) -> u32 {
    let delta = vec3<i32>(left) - vec3<i32>(right);
    let squared = delta * delta;
    return u32(squared.x * 4 + squared.y * 7 + squared.z);
}

fn evaluate_colour(
    texels: array<u32, 16>,
    transparent: bool,
    first: u32,
    second: u32,
) -> ColourBlock {
    var c0 = first;
    var c1 = second;
    var points = 4u;
    if (transparent) {
        if (c0 > c1) {
            let swap = c0;
            c0 = c1;
            c1 = swap;
        }
        points = 3u;
    } else if (c0 < c1) {
        let swap = c0;
        c0 = c1;
        c1 = swap;
    }

    let e0 = expand_565(c0);
    let e1 = expand_565(c1);
    var palette: array<vec3<u32>, 4>;
    palette[0] = e0;
    palette[1] = e1;
    if (points == 3u) {
        palette[2] = (e0 + e1) / 2u;
        palette[3] = vec3<u32>(0u, 0u, 0u);
    } else {
        palette[2] = (2u * e0 + e1) / 3u;
        palette[3] = (e0 + 2u * e1) / 3u;
    }

    var indices = 0u;
    var error = 0u;
    for (var i = 0u; i < 16u; i = i + 1u) {
        let texel = texels[i];
        if (transparent && alpha(texel) < 127u) {
            indices = indices | (3u << (2u * i));
            continue;
        }
        let colour = vec3<u32>(red(texel), green(texel), blue(texel));
        var best = 0u;
        var best_error = 0xffffffffu;
        for (var point = 0u; point < points; point = point + 1u) {
            let candidate = distance(colour, palette[point]);
            if (candidate < best_error) {
                best_error = candidate;
                best = point;
            }
        }
        error = error + best_error;
        indices = indices | (best << (2u * i));
    }
    return ColourBlock(c0, c1, indices, error);
}

fn best_colour(
    texels: array<u32, 16>,
    transparent: bool,
    unique: array<u32, 16>,
    count: u32,
    refine: u32,
) -> ColourBlock {
    var best = ColourBlock(0u, 0u, 0u, 0xffffffffu);
    for (var i = 0u; i < count; i = i + 1u) {
        for (var j = i; j < count; j = j + 1u) {
            let candidate = evaluate_colour(texels, transparent, unique[i], unique[j]);
            if (candidate.error < best.error) {
                best = candidate;
            }
        }
    }

    for (var round = 0u; round < refine; round = round + 1u) {
        var improved = false;
        for (var dimension = 0u; dimension < 6u; dimension = dimension + 1u) {
            for (var side = 0u; side < 2u; side = side + 1u) {
                var delta = 1;
                if (side == 0u) {
                    delta = -1;
                }
                var first = best.c0;
                var second = best.c1;
                if (dimension < 3u) {
                    first = shift_code(first, dimension, delta);
                } else {
                    second = shift_code(second, dimension - 3u, delta);
                }
                let candidate = evaluate_colour(texels, transparent, first, second);
                if (candidate.error < best.error) {
                    best = candidate;
                    improved = true;
                }
            }
        }
        if (!improved) {
            break;
        }
    }
    return best;
}

fn evaluate_alpha(alphas: array<u32, 16>, first: u32, second: u32) -> AlphaBlock {
    var a0 = first;
    var a1 = second;
    var palette: array<u32, 8>;
    palette[0] = a0;
    palette[1] = a1;
    if (a0 > a1) {
        for (var step = 1u; step <= 6u; step = step + 1u) {
            palette[step + 1u] = ((7u - step) * a0 + step * a1) / 7u;
        }
    } else {
        for (var step = 1u; step <= 4u; step = step + 1u) {
            palette[step + 1u] = ((5u - step) * a0 + step * a1) / 5u;
        }
        palette[6] = 0u;
        palette[7] = 255u;
    }

    var low = a0 | (a1 << 8u);
    var high = 0u;
    var error = 0u;
    for (var i = 0u; i < 16u; i = i + 1u) {
        let value = i32(alphas[i]);
        var best = 0u;
        var best_error = 0xffffffffu;
        for (var point = 0u; point < 8u; point = point + 1u) {
            let delta = value - i32(palette[point]);
            let candidate = u32(delta * delta);
            if (candidate < best_error) {
                best_error = candidate;
                best = point;
            }
        }
        error = error + best_error;
        if (i < 5u) {
            low = low | (best << (16u + i * 3u));
        } else if (i > 5u) {
            high = high | (best << (2u + (i - 6u) * 3u));
        } else {
            low = low | ((best & 1u) << 31u);
            high = high | ((best & 6u) >> 1u);
        }
    }
    return AlphaBlock(a0, a1, low, high, error);
}

fn shift_alpha(code: u32, delta: i32) -> u32 {
    return u32(clamp(i32(code) + delta, 0, 255));
}

fn best_alpha(alphas: array<u32, 16>, refine: u32) -> AlphaBlock {
    var minimum = 255u;
    var maximum = 0u;
    for (var i = 0u; i < 16u; i = i + 1u) {
        minimum = min(minimum, alphas[i]);
        maximum = max(maximum, alphas[i]);
    }

    var best = evaluate_alpha(alphas, maximum, minimum);
    let inverse = evaluate_alpha(alphas, minimum, maximum);
    if (inverse.error < best.error) {
        best = inverse;
    }

    for (var round = 0u; round < refine; round = round + 1u) {
        var improved = false;
        for (var endpoint = 0u; endpoint < 2u; endpoint = endpoint + 1u) {
            for (var shift = 0u; shift < 4u; shift = shift + 1u) {
                var delta = 1;
                if (shift == 0u) {
                    delta = -4;
                } else if (shift == 1u) {
                    delta = -1;
                } else if (shift == 3u) {
                    delta = 4;
                }
                var first = best.a0;
                var second = best.a1;
                if (endpoint == 0u) {
                    first = shift_alpha(first, delta);
                } else {
                    second = shift_alpha(second, delta);
                }
                let candidate = evaluate_alpha(alphas, first, second);
                if (candidate.error < best.error) {
                    best = candidate;
                    improved = true;
                }
            }
        }
        if (!improved) {
            break;
        }
    }
    return best;
}

fn shade_slot(bits: u32) -> u32 {
    if (bits <= 1u) {
        return 0u;
    }
    if (bits <= 4u) {
        return 1u;
    }
    if (bits <= 5u) {
        return 2u;
    }
    return 3u;
}

fn lower_shade(value: u32, bits: u32) -> u32 {
    if (bits >= 8u) {
        return value;
    }
    return shades[(shade_slot(bits) * 2u) * 256u + value];
}

fn upper_shade(value: u32, bits: u32) -> u32 {
    if (bits >= 8u) {
        return value;
    }
    return shades[(shade_slot(bits) * 2u + 1u) * 256u + value];
}

fn abs_component(left: vec3<i32>, right: vec3<i32>) -> i32 {
    let delta = abs(left - right);
    return delta.x + delta.y + delta.z;
}

fn reduce_alpha(value: u32) -> u32 {
    if (params.opaque == 1u || params.alpha_bits >= 8u) {
        return 255u;
    }
    let low = lower_shade(value, params.alpha_bits);
    let high = upper_shade(value, params.alpha_bits);
    if (abs(i32(value) - i32(low)) > abs(i32(value) - i32(high))) {
        return high;
    }
    return low;
}

fn reduce_pixel(pixel: u32, x: u32, y: u32) -> u32 {
    if (params.true_colour == 1u && params.alpha_bits == 8u && params.opaque == 0u) {
        return (alpha(pixel) << 24u)
            | (red(pixel) << 16u)
            | (green(pixel) << 8u)
            | blue(pixel);
    }

    var alpha_value = 255u;
    if (params.opaque == 0u) {
        alpha_value = reduce_alpha(alpha(pixel));
    }
    let r = red(pixel);
    let g = green(pixel);
    let b = blue(pixel);
    if (params.true_colour == 1u) {
        return (alpha_value << 24u) | (r << 16u) | (g << 8u) | b;
    }

    let lower = vec3<u32>(
        lower_shade(r, params.red_bits),
        lower_shade(g, params.green_bits),
        lower_shade(b, params.blue_bits),
    );
    let upper = vec3<u32>(
        upper_shade(r, params.red_bits),
        upper_shade(g, params.green_bits),
        upper_shade(b, params.blue_bits),
    );
    let source = vec3<i32>(i32(r), i32(g), i32(b));
    let lower_distance = abs_component(source, vec3<i32>(lower));
    let upper_distance = abs_component(source, vec3<i32>(upper));

    var near = lower;
    var far = upper;
    if (upper_distance < lower_distance) {
        near = upper;
        far = lower;
    }

    var colour = near;
    if (params.dither == 1u && params.packed_dithers == 1u) {
        var best_rung = 0u;
        var best_distance = 0x7fffffff;
        for (var rung = 0u; rung < 17u; rung = rung + 1u) {
            var total = 0;
            for (var channel = 0u; channel < 3u; channel = channel + 1u) {
                let low = i32(near[channel]);
                let high = i32(far[channel]);
                let ladder = low * 16 + (high - low) * i32(rung);
                total = total + abs(ladder - i32(source[channel]) * 16);
            }
            if (total < best_distance) {
                best_distance = total;
                best_rung = rung;
            }
        }
        let column = (y % 4u) * 4u + (x % 4u);
        if (dither_matrix[best_rung * 16u + column] == 1u) {
            colour = far;
        }
    }

    return (alpha_value << 24u) | (colour.x << 16u) | (colour.y << 8u) | colour.z;
}

@compute @workgroup_size(8, 8)
fn encode_blocks(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.blocks_x || id.y >= params.blocks_y) {
        return;
    }

    var texels: array<u32, 16>;
    var unique: array<u32, 16>;
    var unique_count = 0u;
    var transparent = false;
    for (var py = 0u; py < 4u; py = py + 1u) {
        for (var px = 0u; px < 4u; px = px + 1u) {
            let x = id.x * 4u + px;
            let y = id.y * 4u + py;
            var texel = 0u;
            if (x < params.width && y < params.height) {
                texel = source[y * params.width + x];
            }
            texels[py * 4u + px] = texel;
            if (params.kind == 0u && alpha(texel) < 127u) {
                transparent = true;
            } else {
                let colour = to_565(red(texel), green(texel), blue(texel));
                var seen = false;
                for (var i = 0u; i < unique_count; i = i + 1u) {
                    if (unique[i] == colour) {
                        seen = true;
                    }
                }
                if (!seen) {
                    unique[unique_count] = colour;
                    unique_count = unique_count + 1u;
                }
            }
        }
    }

    let block = id.y * params.blocks_x + id.x;
    if (unique_count == 0u) {
        destination[block * 2u] = 0u;
        destination[block * 2u + 1u] = 0xffffffffu;
        return;
    }

    let colour = best_colour(texels, transparent, unique, unique_count, params.refine);
    if (params.kind == 1u) {
        var alphas: array<u32, 16>;
        for (var i = 0u; i < 16u; i = i + 1u) {
            alphas[i] = alpha(texels[i]);
        }
        let alpha_block = best_alpha(alphas, params.refine);
        destination[block * 4u] = alpha_block.low;
        destination[block * 4u + 1u] = alpha_block.high;
        destination[block * 4u + 2u] = colour.c0 | (colour.c1 << 16u);
        destination[block * 4u + 3u] = colour.indices;
    } else {
        destination[block * 2u] = colour.c0 | (colour.c1 << 16u);
        destination[block * 2u + 1u] = colour.indices;
    }
}

@compute @workgroup_size(8, 8)
fn encode_pixels(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.width || id.y >= params.height) {
        return;
    }
    let index = id.y * params.width + id.x;
    destination[index] = reduce_pixel(source[index], id.x, id.y);
}
