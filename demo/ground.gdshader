shader_type spatial;

// Ground material for the planet. Colour comes from the simulation, as a
// latitude-longitude map of the whole planet that is refreshed while the game runs.

uniform sampler2D u_surface : source_color, filter_linear, repeat_enable;
uniform sampler2D u_detail : filter_linear_mipmap, repeat_enable;

// Set per mesh block by Voxel Tools.
uniform mat4 u_block_local_transform;
uniform int u_transition_mask;

varying vec3 v_planet_pos;
varying vec3 v_planet_normal;

// Moves vertices on the border between two levels of detail so the meshes meet without
// gaps. Adapted from the transvoxel shader include that ships with Voxel Tools (MIT).
vec3 transvoxel_position(vec3 vertex_pos, vec4 fdata) {
	int idata = floatBitsToInt(fdata.a);
	int cell_border_mask = idata & 63;
	int vertex_border_mask = (idata >> 8) & 63;
	float t = float((u_transition_mask & cell_border_mask) != 0);
	t *= float((vertex_border_mask & ~u_transition_mask) == 0);
	vec3 pos = mix(vertex_pos, fdata.xyz, t);
	int itransition = (idata >> 16) & 0xff;
	pos *= float(itransition == 0 || (itransition & u_transition_mask) != 0);
	return pos;
}

void vertex() {
	VERTEX = transvoxel_position(VERTEX, CUSTOM0);
	v_planet_pos = (u_block_local_transform * vec4(VERTEX, 1.0)).xyz;
	v_planet_normal = NORMAL;
}

float detail(vec3 p, vec3 weights) {
	return texture(u_detail, p.xy).r * weights.z
		+ texture(u_detail, p.xz).r * weights.y
		+ texture(u_detail, p.zy).r * weights.x;
}

void fragment() {
	vec3 up = normalize(v_planet_pos);
	vec2 uv = vec2(atan(up.x, up.z) / TAU + 0.5, asin(clamp(up.y, -1.0, 1.0)) / PI + 0.5);
	vec3 colour = texture(u_surface, uv).rgb;

	vec3 n = normalize(v_planet_normal);
	vec3 weights = abs(n);
	weights /= weights.x + weights.y + weights.z;
	float fine = detail(v_planet_pos * 0.35, weights);
	float coarse = detail(v_planet_pos * 0.02, weights);
	colour *= mix(0.72, 1.18, fine * 0.5 + coarse * 0.5);

	// Bare rock shows through on steep ground.
	float steep = 1.0 - smoothstep(0.55, 0.8, dot(n, up));
	colour = mix(colour, vec3(0.3, 0.27, 0.25) * (0.7 + 0.5 * fine), steep);

	ALBEDO = colour;
	ROUGHNESS = 0.95;
}
