struct Ray {
    origin: vec3<f32>,
    direction: vec3<f32>,
}

struct Sphere {
    position: vec3<f32>,
    radius: f32,
    color: vec3<f32>,
}

// vec4s are just so alignment isnt messed up on the rust side
struct Camera {
    position: vec4<f32>,
    forward: vec4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
}

@group(1) @binding(0)
var<uniform> camera: Camera;

@group(0)
@binding(0)
var output_texture: texture_storage_2d<rgba8unorm, write>;

const max_distance: f32 = 1000.0;
const min_distance: f32 = 0.01;

var<private> sphere_count: u32;
var<private> spheres: array<Sphere, 2>;

fn sphere_sdf(position: vec3<f32>, sphere: Sphere) -> f32 {
    return length(sphere.position - position) - sphere.radius;
}

fn sdf(position: vec3<f32>) -> f32 {
    if sphere_count == 0u {
        return 0.0;
    }

    var closest_sphere = 0u;
    var dist = sphere_sdf(position, spheres[0]);
    for (var i: u32 = 1u; i < sphere_count; i++) {
        let new_dist = sphere_sdf(position, spheres[i]);
        if new_dist < dist {
            closest_sphere = i;
            dist = new_dist;
        }
    }
    return dist;
}

fn get_normal(p: vec3<f32>) -> vec3<f32> {
    return normalize(vec3<f32>(
        sdf(vec3<f32>(p.x + min_distance, p.y, p.z)) - sdf(vec3<f32>(p.x - min_distance, p.y, p.z)),
        sdf(vec3<f32>(p.x, p.y + min_distance, p.z)) - sdf(vec3<f32>(p.x, p.y - min_distance, p.z)),
        sdf(vec3<f32>(p.x, p.y, p.z + min_distance)) - sdf(vec3<f32>(p.x, p.y, p.z - min_distance))
    ));
}

fn does_hit(ray: Ray) -> bool {
    var ray = ray;

    var distance: f32 = 0.0;
    while distance < max_distance {
        var dist = sdf(ray.origin);
        ray.origin += ray.direction * dist;
        distance += dist;
        if dist < min_distance {
            return true;
        }
    }
    return false;
}

fn get_color(ray: Ray) -> vec3<f32> {
    var ray = ray;

    if sphere_count != 0u {
        var distance: f32 = 0.0;
        while distance < max_distance {
            var closest_sphere = 0u;
            var dist = sphere_sdf(ray.origin, spheres[0]);
            for (var i: u32 = 1u; i < sphere_count; i++) {
                let new_dist = sphere_sdf(ray.origin, spheres[i]);
                if new_dist < dist {
                    closest_sphere = i;
                    dist = new_dist;
                }
            }
            ray.origin += ray.direction * dist;
            distance += dist;
            if dist < min_distance {
                let light_direction = normalize(vec3<f32>(0.3, -1.0, 0.4));

                let normal = get_normal(ray.origin);

                var new_ray: Ray;
                new_ray.origin = ray.origin + normal * min_distance * 2.0;
                new_ray.direction = -light_direction;
                let does_hit = does_hit(new_ray);

                let light_amount = max(f32(!does_hit) * dot(normal, -light_direction), 0.05);
                return spheres[closest_sphere].color * light_amount;
            }
        }
    }

    let t = ray.direction.y * 0.5 + 0.5;
    let up_color = vec3<f32>(1.0, 1.0, 1.0);
    let down_color = vec3<f32>(0.5, 0.7, 1.0);
    return up_color * (1.0 - t) + down_color * t;
}

@compute
@workgroup_size(16, 16)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>
) {
    let size = textureDimensions(output_texture);
    let coords = vec2<i32>(global_id.xy);

    if coords.x >= size.x || coords.y >= size.y {
        return;
    }

    sphere_count = 2u;
    spheres[0].position = vec3<f32>(0.0, 0.0, 3.0);
    spheres[0].radius = 1.0;
    spheres[0].color = vec3<f32>(0.2, 0.3, 0.8);
    spheres[1].position = vec3<f32>(0.2, 1.3, 2.5);
    spheres[1].radius = 0.3;
    spheres[1].color = vec3<f32>(0.8, 0.5, 0.2);

    var uv = vec2<f32>(coords) / vec2<f32>(size);
    uv.y = 1.0 - uv.y;
    uv = uv * 2.0 - 1.0;

    let aspect = f32(size.x) / f32(size.y);

    var ray: Ray;
    ray.origin = camera.position.xyz;
    ray.direction = normalize(camera.right.xyz * uv.x * aspect + camera.up.xyz * uv.y + camera.forward.xyz);

    let color = get_color(ray);
    textureStore(output_texture, coords.xy, vec4<f32>(color, 1.0));
}
