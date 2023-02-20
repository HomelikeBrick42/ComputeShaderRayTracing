struct Ray {
    origin: vec3<f32>,
    direction: vec3<f32>,
}

struct Camera {
    position: vec3<f32>,
    forward: vec3<f32>,
    right: vec3<f32>,
    up: vec3<f32>,
    up_sky_color: vec3<f32>,
    down_sky_color: vec3<f32>,
    min_distance: f32,
    max_distance: f32,
}

struct Sphere {
    position: vec3<f32>,
    radius: f32,
    color: vec3<f32>,
}

struct SpheresBuffer {
    sphere_count: u32,
    spheres: array<Sphere>,
}

@group(0)
@binding(0)
var output_texture: texture_storage_2d<rgba8unorm, write>;

@group(1)
@binding(0)
var<uniform> camera: Camera;

@group(2)
@binding(0)
var<storage> spheres_storage: SpheresBuffer;

fn sphere_sdf(position: vec3<f32>, sphere: Sphere) -> f32 {
    return distance(position, sphere.position) - sphere.radius;
}

fn sdf(position: vec3<f32>) -> f32 {
    if spheres_storage.sphere_count == 0u {
        return 0.0;
    }

    var closest_sphere = 0u;
    var dist = sphere_sdf(position, spheres_storage.spheres[0]);
    for (var i: u32 = 1u; i < spheres_storage.sphere_count; i++) {
        let new_dist = sphere_sdf(position, spheres_storage.spheres[i]);
        if new_dist < dist {
            closest_sphere = i;
            dist = new_dist;
        }
    }
    return dist;
}

fn get_normal(p: vec3<f32>) -> vec3<f32> {
    return normalize(vec3<f32>(
        sdf(vec3<f32>(p.x + camera.min_distance, p.y, p.z)) - sdf(vec3<f32>(p.x - camera.min_distance, p.y, p.z)),
        sdf(vec3<f32>(p.x, p.y + camera.min_distance, p.z)) - sdf(vec3<f32>(p.x, p.y - camera.min_distance, p.z)),
        sdf(vec3<f32>(p.x, p.y, p.z + camera.min_distance)) - sdf(vec3<f32>(p.x, p.y, p.z - camera.min_distance))
    ));
}

fn does_hit(ray: Ray) -> bool {
    var ray = ray;

    var distance: f32 = 0.0;
    while distance < camera.max_distance {
        var dist = sdf(ray.origin);
        ray.origin += ray.direction * dist;
        distance += dist;
        if dist < camera.min_distance {
            return true;
        }
    }
    return false;
}

fn get_color(ray: Ray) -> vec3<f32> {
    var ray = ray;

    if spheres_storage.sphere_count != 0u {
        var distance: f32 = 0.0;
        while distance < camera.max_distance {
            var closest_sphere = 0u;
            var dist = sphere_sdf(ray.origin, spheres_storage.spheres[0]);
            for (var i: u32 = 1u; i < spheres_storage.sphere_count; i++) {
                let new_dist = sphere_sdf(ray.origin, spheres_storage.spheres[i]);
                if new_dist < dist {
                    closest_sphere = i;
                    dist = new_dist;
                }
            }
            ray.origin += ray.direction * dist;
            distance += dist;
            if dist < camera.min_distance {
                let light_direction = normalize(vec3<f32>(0.3, -1.0, 0.4));

                let normal = get_normal(ray.origin);

                var new_ray: Ray;
                new_ray.origin = ray.origin + normal * camera.min_distance * 2.0;
                new_ray.direction = -light_direction;
                let does_hit = does_hit(new_ray);

                let light_amount = max(f32(!does_hit) * dot(normal, -light_direction), 0.05);
                return spheres_storage.spheres[closest_sphere].color * light_amount;
            }
        }
    }

    let t = ray.direction.y * 0.5 + 0.5;
    return camera.up_sky_color * (1.0 - t) + camera.down_sky_color * t;
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

    var uv = vec2<f32>(coords) / vec2<f32>(size);
    uv.y = 1.0 - uv.y;
    uv = uv * 2.0 - 1.0;

    let aspect = f32(size.x) / f32(size.y);

    var ray: Ray;
    ray.origin = camera.position;
    ray.direction = normalize(camera.right * uv.x * aspect + camera.up * uv.y + camera.forward);

    let color = get_color(ray);
    textureStore(output_texture, coords.xy, vec4<f32>(color, 1.0));
}
