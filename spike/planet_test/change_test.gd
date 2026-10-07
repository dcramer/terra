extends Node

# Spike test for question 2 in docs/prototypes.md: can the terrain change at runtime
# (as terraforming would change it) while a player's dug hole survives?
#
# It digs a hole, then grows the planet by GROWTH metres, and reports what happens to
# untouched ground far away, to ground right next to the hole, and to the hole itself.

const Planet = preload("res://planet.gd")

const DIG_RADIUS := 6.0
const GROWTH := 5.0
const FAR_DIR := Vector3(0.6, 0.8, 0.0)
const NEAR_OFFSETS := [8.0, 10.0, 12.0, 14.0, 20.0, 30.0]


func _ready() -> void:
	var terrain := await _new_terrain(Planet.RADIUS)
	var tool := _tool_for(terrain)
	var dig := Vector3.UP * Planet.surface_distance(tool, Vector3.UP, Planet.RADIUS)
	print("ground at dig site: ", dig.y)
	print("--- before any change")
	_report(tool, dig)
	tool.do_sphere(dig, DIG_RADIUS)
	print("--- after digging")
	_report(tool, dig)

	# Approach A: change the generator's input in place and recompile.
	var gen: VoxelGeneratorGraph = terrain.generator
	var g := gen.get_main_function()
	g.set_node_default_input_by_name(g.find_node_by_name("sphere"), "radius", Planet.RADIUS + GROWTH)
	print("recompile: ", gen.compile().get("success"))
	await get_tree().create_timer(1.0).timeout
	print("--- A: generator input changed in place (+%s m)" % GROWTH)
	_report(tool, dig)

	# Approach B: assign a new generator to the same terrain.
	terrain.generator = Planet.make_generator(Planet.RADIUS + GROWTH * 2.0)
	await get_tree().create_timer(1.0).timeout
	tool = _tool_for(terrain)
	print("--- B: new generator assigned to the same terrain (+%s m)" % (GROWTH * 2.0))
	_report(tool, dig)

	# Approach C: treat the dig as an operation. Build fresh terrain from the changed
	# generator and replay the operation on top.
	terrain.queue_free()
	terrain = await _new_terrain(Planet.RADIUS + GROWTH)
	tool = _tool_for(terrain)
	print("--- C: fresh terrain (+%s m), before replaying the dig" % GROWTH)
	_report(tool, dig)
	tool.do_sphere(dig, DIG_RADIUS)
	print("--- C: fresh terrain (+%s m), after replaying the dig" % GROWTH)
	_report(tool, dig)

	# Cost of replaying many operations, which is also the cost of a late join.
	var count := 2000
	var rng := RandomNumberGenerator.new()
	rng.seed = 1
	var start := Time.get_ticks_msec()
	for i in count:
		var dir := Vector3(rng.randf_range(-1, 1), rng.randf_range(-1, 1), rng.randf_range(-1, 1)).normalized()
		tool.do_sphere(dir * (Planet.RADIUS + GROWTH), DIG_RADIUS)
	var elapsed := Time.get_ticks_msec() - start
	print("--- replay cost: %d digs in %d ms (%.2f ms each)" % [count, elapsed, float(elapsed) / count])
	print("edited data blocks held in memory: ", terrain.debug_get_data_block_count())
	get_tree().quit()


func _new_terrain(radius: float) -> VoxelLodTerrain:
	var terrain := Planet.make_terrain(radius)
	add_child(terrain)
	await get_tree().create_timer(1.0).timeout
	return terrain


func _tool_for(terrain: VoxelLodTerrain) -> VoxelTool:
	var tool := terrain.get_voxel_tool()
	tool.channel = VoxelBuffer.CHANNEL_SDF
	tool.mode = VoxelTool.MODE_REMOVE
	return tool


# Ground height along a vertical column at a sideways offset from the dig site.
func _column_height(tool: VoxelTool, x: float) -> int:
	var y := int(Planet.RADIUS * 1.5)
	while y > 0:
		if tool.get_voxel_f(Vector3i(int(x), y, 0)) < 0.0:
			return y
		y -= 1
	return -1


func _report(tool: VoxelTool, dig: Vector3) -> void:
	var far := Planet.surface_distance(tool, FAR_DIR.normalized(), Planet.RADIUS)
	var hole := tool.get_voxel_f(Vector3i(dig))
	var near := NEAR_OFFSETS.map(func(x): return _column_height(tool, x))
	print("  far ground height: ", far)
	print("  dig centre: ", "air (hole)" if hole > 0.0 else "solid", " (", snappedf(hole, 0.01), ")")
	print("  ground height at ", NEAR_OFFSETS, " m from the hole: ", near)
