extends RefCounted

# A bare procedural planet: a noisy sphere on level-of-detail smooth voxel terrain.
# Shared by the spike tests so every process builds an identical one.

const RADIUS := 200.0
const NOISE_AMPLITUDE := 12.0
const NOISE_SEED := 1337


static func make_generator(radius: float) -> VoxelGeneratorGraph:
	var gen := VoxelGeneratorGraph.new()
	var g := gen.get_main_function()
	var x := g.create_node(VoxelGraphFunction.NODE_INPUT_X, Vector2(0, 0))
	var y := g.create_node(VoxelGraphFunction.NODE_INPUT_Y, Vector2(0, 100))
	var z := g.create_node(VoxelGraphFunction.NODE_INPUT_Z, Vector2(0, 200))

	var sphere := g.create_node(VoxelGraphFunction.NODE_SDF_SPHERE, Vector2(200, 0))
	g.set_node_name(sphere, "sphere")
	g.set_node_default_input_by_name(sphere, "radius", radius)

	var noise_res := FastNoiseLite.new()
	noise_res.seed = NOISE_SEED
	noise_res.frequency = 4.0 / radius
	var noise := g.create_node(VoxelGraphFunction.NODE_NOISE_3D, Vector2(200, 200))
	g.set_node_param_by_name(noise, "noise", noise_res)

	var scaled := g.create_node(VoxelGraphFunction.NODE_MULTIPLY, Vector2(400, 200))
	g.set_node_default_input(scaled, 1, NOISE_AMPLITUDE * radius / RADIUS)
	var sum := g.create_node(VoxelGraphFunction.NODE_ADD, Vector2(600, 100))
	var out := g.create_node(VoxelGraphFunction.NODE_OUTPUT_SDF, Vector2(800, 100))

	for node in [sphere, noise]:
		g.add_connection(x, 0, node, 0)
		g.add_connection(y, 0, node, 1)
		g.add_connection(z, 0, node, 2)
	g.add_connection(noise, 0, scaled, 0)
	g.add_connection(sphere, 0, sum, 0)
	g.add_connection(scaled, 0, sum, 1)
	g.add_connection(sum, 0, out, 0)

	var result := gen.compile()
	if not result.get("success", false):
		push_error("Planet graph failed to compile: %s" % result)
	return gen


static func make_terrain(radius: float) -> VoxelLodTerrain:
	var terrain := VoxelLodTerrain.new()
	terrain.generator = make_generator(radius)
	terrain.mesher = VoxelMesherTransvoxel.new()
	var half := 256.0
	while radius * 1.2 >= half:
		half *= 2.0
	terrain.voxel_bounds = AABB(Vector3.ONE * -half, Vector3.ONE * half * 2.0)
	# Same level-of-detail settings as the Voxel Tools solar-system demo, with extra
	# levels for planets larger than 2 km.
	terrain.lod_count = 7 + maxi(0, ceili(log(radius / 2000.0) / log(2.0)))
	terrain.lod_distance = 60.0
	terrain.mesh_block_size = 32
	terrain.view_distance = int(radius * 10.0)
	terrain.generate_collisions = false
	# Lets edits and queries happen anywhere, not only near a viewer.
	terrain.full_load_mode_enabled = true
	return terrain


# Height of the ground above the planet's centre along a direction, found by marching
# down until the signed distance goes negative (solid).
static func surface_distance(tool: VoxelTool, dir: Vector3, radius: float) -> float:
	var d := radius * 1.5
	while d > 0.0:
		if tool.get_voxel_f(Vector3i((dir * d).round())) < 0.0:
			return d
		d -= 1.0
	return -1.0
