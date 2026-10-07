extends RefCounted

# Builds the planet's voxel terrain so that its shape matches the simulation's idea of
# the ground: the same waves, plus a little fine noise the simulation doesn't see.

const DETAIL_AMPLITUDE_M := 2.0


static func make(sim: RefCounted, radius: float, material: Material) -> VoxelLodTerrain:
	var terrain := VoxelLodTerrain.new()
	terrain.generator = _generator(sim, radius)
	terrain.mesher = VoxelMesherTransvoxel.new()
	terrain.material = material
	var half := 256.0
	while radius * 1.2 >= half:
		half *= 2.0
	terrain.voxel_bounds = AABB(Vector3.ONE * -half, Vector3.ONE * half * 2.0)
	terrain.lod_count = 7 + maxi(0, ceili(log(radius / 2000.0) / log(2.0)))
	terrain.lod_distance = 60.0
	terrain.mesh_block_size = 32
	terrain.view_distance = int(radius * 10.0)
	terrain.generate_collisions = true
	terrain.collision_lod_count = 2
	# Lets digging and ground queries work anywhere, not only near a viewer.
	terrain.full_load_mode_enabled = true
	return terrain


static func _generator(sim: RefCounted, radius: float) -> VoxelGeneratorGraph:
	var waves: PackedFloat32Array = sim.terrain_waves()
	var terms := PackedStringArray()
	for i in range(0, waves.size(), 6):
		terms.append("%.6f*sin(%.6f*(0%s%s%s)+%.6f)" % [
			waves[i + 5], waves[i + 3], _signed(waves[i], "nx"), _signed(waves[i + 1], "ny"),
			_signed(waves[i + 2], "nz"), waves[i + 4]])

	var gen := VoxelGeneratorGraph.new()
	var g := gen.get_main_function()
	var x := g.create_node(VoxelGraphFunction.NODE_INPUT_X, Vector2(0, 0))
	var y := g.create_node(VoxelGraphFunction.NODE_INPUT_Y, Vector2(0, 100))
	var z := g.create_node(VoxelGraphFunction.NODE_INPUT_Z, Vector2(0, 200))

	# Unit direction from the centre, and distance from it.
	var norm := g.create_node(VoxelGraphFunction.NODE_NORMALIZE_3D, Vector2(200, 0))
	var shape := g.create_node(VoxelGraphFunction.NODE_EXPRESSION, Vector2(400, 0))
	g.set_node_param_by_name(shape, "expression", "len-%.3f-(%s)" % [radius, "+".join(terms)])
	g.set_expression_node_inputs(shape, PackedStringArray(["nx", "ny", "nz", "len"]))

	var noise_res := FastNoiseLite.new()
	noise_res.seed = 99
	noise_res.frequency = 0.03
	var noise := g.create_node(VoxelGraphFunction.NODE_NOISE_3D, Vector2(400, 300))
	g.set_node_param_by_name(noise, "noise", noise_res)
	var bumps := g.create_node(VoxelGraphFunction.NODE_MULTIPLY, Vector2(600, 300))
	g.set_node_default_input(bumps, 1, DETAIL_AMPLITUDE_M)
	var sum := g.create_node(VoxelGraphFunction.NODE_ADD, Vector2(800, 100))
	var out := g.create_node(VoxelGraphFunction.NODE_OUTPUT_SDF, Vector2(1000, 100))

	for node in [norm, noise]:
		g.add_connection(x, 0, node, 0)
		g.add_connection(y, 0, node, 1)
		g.add_connection(z, 0, node, 2)
	for port in 4:
		g.add_connection(norm, port, shape, port)
	g.add_connection(noise, 0, bumps, 0)
	g.add_connection(shape, 0, sum, 0)
	g.add_connection(bumps, 0, sum, 1)
	g.add_connection(sum, 0, out, 0)

	var result := gen.compile()
	if not result.get("success", false):
		push_error("Planet terrain graph failed to compile: %s" % result)
	return gen


# One "+c*name" or "-c*name" term. The graph's expression parser has no unary minus and
# misreads a name followed by "*(" as a function call, so signs are written as subtraction
# and the constant always comes first.
static func _signed(value: float, name: String) -> String:
	return "%s%.6f*%s" % ["+" if value >= 0.0 else "-", absf(value), name]


# Distance from the planet's centre to the ground along a direction, from voxel data.
static func ground_distance(tool: VoxelTool, dir: Vector3, radius: float) -> float:
	var d := radius + 400.0
	while d > radius - 400.0:
		if tool.get_voxel_f(Vector3i((dir * d).round())) < 0.0:
			# Refine within the last metre.
			for i in 10:
				d += 0.1
				if tool.get_voxel_f_interpolated(dir * d) >= 0.0:
					break
			return d
		d -= 1.0
	return radius
