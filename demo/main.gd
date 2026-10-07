extends Node3D

# Tech demo: stand on a voxel planet, dig it, place terraforming machines, and watch the
# simulation change the sea, the ground and the sky around you. Fly up to see it from orbit.
#
#   W A S D, mouse   move, look        Shift   run / fly fast
#   Space, Ctrl      jump / up, down   F       toggle flying
#   Left click       dig               Right click   fill
#   1 2 3            pick heater, gas generator, seeder       E   place it
#   - =              simulation slower, faster                Esc release the mouse
#
# For automated runs:  -- --tour   (screenshots and frame times, no input needed)

const Terrain = preload("res://terrain.gd")
const Player = preload("res://player.gd")

const RADIUS_M := 2000.0
const SUBDIVISIONS := 5
const SURFACE_TEXTURE_SIZE := Vector2i(512, 256)
const REACH_M := 10.0
const DIG_RADIUS_M := 2.2
const SUN_DIRECTION := Vector3(0.35, 0.4, 0.85)
const MACHINES := ["Heater", "Gas generator", "Seeder"]
const MACHINE_COLOURS := [Color(1.0, 0.35, 0.1), Color(1.0, 0.9, 0.2), Color(0.3, 1.0, 0.3)]
const PHASES := ["Dead", "Thawing", "Wet", "Green", "Breathable"]
const SPEEDS := [0, 1, 5, 25, 100]

var _sim: RefCounted
var _terrain: VoxelLodTerrain
var _tool: VoxelToolLodTerrain
var _player: CharacterBody3D
var _sea: MeshInstance3D
var _environment := Environment.new()
var _surface_texture: ImageTexture
var _label := Label.new()
var _machine := 0
var _speed := 1
var _since_refresh := 1.0
var _summary: PackedFloat32Array
var _ticks_per_second := 0.0
var _refresh_cost_ms := 0.0


func _ready() -> void:
	_sim = ClassDB.instantiate("TerraSim")
	_sim.setup(SUBDIVISIONS, 1)

	var sun := DirectionalLight3D.new()
	sun.look_at_from_position(SUN_DIRECTION.normalized(), Vector3.ZERO, Vector3.UP)
	sun.shadow_enabled = true
	sun.directional_shadow_max_distance = 400.0
	add_child(sun)
	_environment.background_mode = Environment.BG_COLOR
	_environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	_environment.fog_enabled = true
	_environment.tonemap_mode = Environment.TONE_MAPPER_FILMIC
	var world_environment := WorldEnvironment.new()
	world_environment.environment = _environment
	add_child(world_environment)

	_surface_texture = ImageTexture.create_from_image(_surface_image())
	var detail := NoiseTexture2D.new()
	detail.noise = FastNoiseLite.new()
	detail.seamless = true
	detail.width = 256
	detail.height = 256
	var ground := ShaderMaterial.new()
	ground.shader = preload("res://ground.gdshader")
	ground.set_shader_parameter("u_surface", _surface_texture)
	ground.set_shader_parameter("u_detail", detail)

	_terrain = Terrain.make(_sim, RADIUS_M, ground)
	add_child(_terrain)
	_tool = _terrain.get_voxel_tool()
	_tool.channel = VoxelBuffer.CHANNEL_SDF

	# One sea for the whole planet. It starts below the lowest ground and rises as ice melts.
	_sea = MeshInstance3D.new()
	var sphere := SphereMesh.new()
	sphere.radius = 1.0
	sphere.height = 2.0
	sphere.radial_segments = 256
	sphere.rings = 128
	var water := StandardMaterial3D.new()
	water.albedo_color = Color(0.04, 0.17, 0.4)
	water.roughness = 0.12
	water.metallic = 0.25
	sphere.material = water
	_sea.mesh = sphere
	add_child(_sea)

	_player = Player.new()
	_player.ground_tool = _tool
	add_child(_player)
	var spawn := _spawn_direction()
	var ground_point := spawn * Terrain.ground_distance(_tool, spawn, RADIUS_M)
	_player.place(ground_point + spawn * 0.2, ground_point + SUN_DIRECTION.cross(spawn) * 10.0)

	var layer := CanvasLayer.new()
	_label.position = Vector2(16, 12)
	_label.add_theme_font_size_override("font_size", 17)
	_label.add_theme_color_override("font_outline_color", Color.BLACK)
	_label.add_theme_constant_override("outline_size", 5)
	layer.add_child(_label)
	var crosshair := Label.new()
	crosshair.text = "+"
	crosshair.set_anchors_preset(Control.PRESET_CENTER)
	layer.add_child(crosshair)
	add_child(layer)

	_refresh()
	_set_speed(_speed)
	if OS.get_cmdline_user_args().has("--tour"):
		_tour()
	elif OS.get_cmdline_user_args().has("--stress"):
		_stress()
	elif OS.get_cmdline_user_args().has("--lag"):
		_lag_check()
	elif DisplayServer.get_name() != "headless":
		Input.mouse_mode = Input.MOUSE_MODE_CAPTURED


func _process(delta: float) -> void:
	_since_refresh += delta
	if _since_refresh >= 0.25:
		var began := Time.get_ticks_usec()
		_refresh()
		_refresh_cost_ms = maxf(_refresh_cost_ms, (Time.get_ticks_usec() - began) / 1000.0)
	_update_sky()


# The simulation runs on its own thread, 60 ticks a second at x1, so it never costs a frame.
func _set_speed(index: int) -> void:
	_speed = clampi(index, 0, SPEEDS.size() - 1)
	_sim.set_rate(60 * SPEEDS[_speed])


# Pushes the simulation's state into what the player sees.
func _refresh() -> void:
	var s: PackedFloat32Array = _sim.summary()
	if _summary.size() > 0 and _since_refresh > 0.0:
		_ticks_per_second = (s[0] - _summary[0]) / _since_refresh
	_summary = s
	_since_refresh = 0.0
	_surface_texture.update(_surface_image())
	_sea.scale = Vector3.ONE * (RADIUS_M + s[3])

	var here: PackedFloat32Array = _sim.cell_values(_sim.cell_at(_player.global_position))
	var altitude := _player.global_position.length() - RADIUS_M
	_label.text = "%s   |   simulation x%d (%d ticks/s)   |   %s   |   %d fps\n" % [
			PHASES[int(s[8])], SPEEDS[_speed], _ticks_per_second,
			"flying" if _player.flying else "walking", Engine.get_frames_per_second()] \
		+ "Planet: air %.2f   temperature %.0f C   sea %.0f%%   ice %.0f%%   life %.0f%%   oxygen %.0f%%\n" % [
			s[1], s[2], s[4] * 100.0, s[5] * 100.0, s[6] * 100.0, s[7] * 100.0] \
		+ "Here: %.0f C   moisture %.0f%%   life %.0f%%   ice %.0f m   altitude %.0f m\n" % [
			here[0], here[1] * 100.0, here[2] * 100.0, here[3], altitude] \
		+ "Machine: %s  (1 2 3 to pick, E to place)" % MACHINES[_machine]


func _surface_image() -> Image:
	return Image.create_from_data(SURFACE_TEXTURE_SIZE.x, SURFACE_TEXTURE_SIZE.y, false,
		Image.FORMAT_RGB8, _sim.surface_texture(SURFACE_TEXTURE_SIZE.x, SURFACE_TEXTURE_SIZE.y))


# The sky is the atmosphere made visible: black on a dead planet, dusty as the air
# thickens, blue once it is thick. It thins back to black as you climb out of it.
func _update_sky() -> void:
	var air := clampf(_summary[1], 0.0, 1.5)
	var altitude := maxf(_player.camera.global_position.length() - RADIUS_M, 0.0)
	var thickness := sqrt(minf(air, 1.0)) * exp(-altitude / (250.0 + 500.0 * air))
	var tint := Color(0.78, 0.58, 0.42).lerp(Color(0.36, 0.58, 0.95), smoothstep(0.25, 0.9, air))
	var sky := Color(0.005, 0.005, 0.012).lerp(tint, thickness)
	_environment.background_color = sky
	_environment.ambient_light_color = Color(0.16, 0.16, 0.2).lerp(
		tint.lerp(Color.WHITE, 0.5), thickness * 0.5)
	_environment.fog_light_color = sky
	_environment.fog_density = 0.0005 * thickness


func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseButton and event.pressed:
		if Input.mouse_mode != Input.MOUSE_MODE_CAPTURED:
			Input.mouse_mode = Input.MOUSE_MODE_CAPTURED
		elif event.button_index == MOUSE_BUTTON_LEFT:
			_edit_ground(VoxelTool.MODE_REMOVE)
		elif event.button_index == MOUSE_BUTTON_RIGHT:
			_edit_ground(VoxelTool.MODE_ADD)
	elif event is InputEventKey and event.pressed and not event.echo:
		match event.keycode:
			KEY_ESCAPE:
				Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
			KEY_1, KEY_2, KEY_3:
				_machine = event.keycode - KEY_1
			KEY_E:
				var hit := _aim()
				if hit != null:
					_place_machine(_machine, hit.position)
			KEY_MINUS:
				_set_speed(_speed - 1)
			KEY_EQUAL:
				_set_speed(_speed + 1)


# What the player is pointing at on the ground, within reach, or null.
func _aim() -> VoxelRaycastResult:
	return _tool.raycast(_player.camera.global_position, _player.look_direction(), REACH_M)


func _edit_ground(mode: int) -> void:
	var hit := _aim()
	if hit == null:
		return
	_tool.mode = mode
	var centre := Vector3(hit.position if mode == VoxelTool.MODE_REMOVE else hit.previous_position)
	_tool.do_sphere(centre, DIG_RADIUS_M)


func _place_machine(machine: int, point: Vector3) -> void:
	_sim.place(machine, _sim.cell_at(point))
	var body := MeshInstance3D.new()
	var mesh := CylinderMesh.new()
	mesh.top_radius = 0.5
	mesh.bottom_radius = 0.8
	mesh.height = 3.0
	var material := StandardMaterial3D.new()
	material.albedo_color = MACHINE_COLOURS[machine]
	material.emission_enabled = true
	material.emission = MACHINE_COLOURS[machine]
	material.emission_energy_multiplier = 0.6
	mesh.material = material
	body.mesh = mesh
	add_child(body)
	var up := point.normalized()
	body.global_position = point + up * 1.5
	body.global_basis = Basis(Quaternion(Vector3.UP, up))


# A spot on the sunlit side, near the equator, on ground that will end up as coast.
func _spawn_direction() -> Vector3:
	var sun := SUN_DIRECTION.normalized()
	var best := sun
	var best_score := -INF
	for cell in _sim.cell_count():
		var values: PackedFloat32Array = _sim.cell_values(cell)
		var dir: Vector3 = _sim.cell_direction(cell)
		if values[4] < 12.0 or values[4] > 40.0 or absf(values[5]) > 0.45:
			continue
		var score := dir.dot(sun) - values[4] * 0.002
		if score > best_score:
			best_score = score
			best = dir
	return best


# Scripted run for checking the demo without a player: terraforms the planet on a fast
# clock, taking screenshots from the same spot on the ground and then from orbit.
func _tour() -> void:
	_player.controls_enabled = false
	_player.flying = true
	_set_speed(0)
	var spawn := _player.global_position.normalized()
	var ground_m := Terrain.ground_distance(_tool, spawn, RADIUS_M)
	var eye := spawn * (ground_m + 0.3)
	var ahead := eye + SUN_DIRECTION.cross(spawn).normalized() * 10.0

	# How well the voxel ground agrees with the simulation's idea of the ground.
	var worst := 0.0
	for cell in range(0, _sim.cell_count(), 257):
		var dir: Vector3 = _sim.cell_direction(cell)
		var expected: float = _sim.cell_values(cell)[4]
		worst = maxf(worst, absf(Terrain.ground_distance(_tool, dir, RADIUS_M) - RADIUS_M - expected))
	print("DEMO terrain and simulation disagree on ground height by at most %.1f m" % worst)

	await _check_controls(spawn, ground_m, ahead)
	_player.controls_enabled = false
	_player.flying = true

	for i in 5:
		_sim.place(1, i)
	for lon in range(0, 360, 30):
		for lat in [-40.0, -15.0, 15.0, 40.0]:
			var d := Vector3(cos(deg_to_rad(lat)) * sin(deg_to_rad(lon)), sin(deg_to_rad(lat)),
				cos(deg_to_rad(lat)) * cos(deg_to_rad(lon)))
			_sim.place(2, _sim.cell_at(d))
	_place_machine(2, _player.global_position + (ahead - eye).normalized() * 6.0)

	_player.place(eye, ahead, -0.12)
	await _shot("1_dead")

	_tool.mode = VoxelTool.MODE_REMOVE
	var hit := _tool.raycast(_player.camera.global_position,
		(_player.look_direction() - spawn * 0.35).normalized(), 30.0)
	if hit != null:
		_tool.do_sphere(Vector3(hit.position), 3.0)
	await _shot("2_dug")

	for stage in [[1500, "3_thawing"], [2500, "4_wet"], [6000, "5_green"]]:
		_sim.step(stage[0])
		await _shot(stage[1])

	_player.place(spawn * RADIUS_M * 1.25, spawn * RADIUS_M + SUN_DIRECTION.cross(spawn) * RADIUS_M, -0.5)
	await _shot("6_high")
	_player.place(SUN_DIRECTION.normalized() * RADIUS_M * 2.6, Vector3.UP * RADIUS_M * 2.6, 0.0)
	_player.look_target = Vector3.ZERO
	await _shot("7_orbit")
	get_tree().quit()


# Exercises walking, gravity and digging through the same code paths a player uses.
func _check_controls(spawn: Vector3, ground_m: float, ahead: Vector3) -> void:
	var height_above_ground := func() -> float:
		var p := _player.global_position
		return p.length() - Terrain.ground_distance(_tool, p.normalized(), RADIUS_M)
	var wait := func(seconds: float) -> void:
		for i in int(seconds * Engine.physics_ticks_per_second):
			await get_tree().physics_frame

	_player.flying = false
	_player.controls_enabled = true
	_player.place(spawn * (ground_m + 20.0), ahead)
	await wait.call(4.0)
	print("DEMO check, dropped from 20 m: came to rest %.2f m above the ground" % height_above_ground.call())

	var before := _player.global_position
	var key := InputEventKey.new()
	key.keycode = KEY_W
	key.pressed = true
	Input.parse_input_event(key)
	await wait.call(3.0)
	key = InputEventKey.new()
	key.keycode = KEY_W
	key.pressed = false
	Input.parse_input_event(key)
	await wait.call(0.5)
	print("DEMO check, walked forward 3 s: moved %.1f m, now %.2f m above the ground" % [
		before.distance_to(_player.global_position), height_above_ground.call()])

	_player.place(_player.global_position, _player.global_position + (ahead - spawn * ground_m), -0.9)
	await wait.call(0.2)
	var hit := _aim()
	if hit == null:
		print("DEMO check, dig: nothing in reach when looking at the ground")
		return
	var site := Vector3(hit.position)
	var solid_before := _tool.get_voxel_f(hit.position) < 0.0
	_edit_ground(VoxelTool.MODE_REMOVE)
	var solid_after := _tool.get_voxel_f(hit.position) < 0.0
	_edit_ground(VoxelTool.MODE_ADD)
	print("DEMO check, dig then fill %.1f m away: solid before=%s, after digging=%s, after filling=%s" % [
		site.distance_to(_player.camera.global_position), solid_before, solid_after,
		_tool.get_voxel_f(hit.position) < 0.0])


# Load test: how much the renderer can draw on top of the terrain. Instanced plants test
# raw throughput; separate objects test the cost of one scene node per thing.
func _stress() -> void:
	_player.controls_enabled = false
	_player.flying = true
	_set_speed(1)
	var up := _player.global_position.normalized()
	var ground_m := Terrain.ground_distance(_tool, up, RADIUS_M)
	var side := SUN_DIRECTION.cross(up).normalized()
	var ahead := up.cross(side)
	_player.place(up * (ground_m + 2.0), up * ground_m + side * 10.0, -0.1)
	for i in 5:
		_sim.place(1, i)
	_sim.step(4000)
	_refresh()
	await _measure("terrain, sea and sky only")

	var plant := ArrayMesh.new()
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = PackedVector3Array([
		Vector3(-0.3, 0, 0), Vector3(0.3, 0, 0), Vector3(0.3, 0.7, 0), Vector3(-0.3, 0.7, 0),
		Vector3(0, 0, -0.3), Vector3(0, 0, 0.3), Vector3(0, 0.7, 0.3), Vector3(0, 0.7, -0.3)])
	arrays[Mesh.ARRAY_NORMAL] = PackedVector3Array([Vector3.UP, Vector3.UP, Vector3.UP, Vector3.UP,
		Vector3.UP, Vector3.UP, Vector3.UP, Vector3.UP])
	arrays[Mesh.ARRAY_INDEX] = PackedInt32Array([0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7])
	plant.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	var leaf := StandardMaterial3D.new()
	leaf.albedo_color = Color(0.2, 0.55, 0.15)
	leaf.cull_mode = BaseMaterial3D.CULL_DISABLED
	plant.surface_set_material(0, leaf)

	var rng := RandomNumberGenerator.new()
	rng.seed = 5
	var upright := Basis(side, up, side.cross(up))
	var scatter := func(count: int, radius: float) -> PackedVector3Array:
		var points := PackedVector3Array()
		points.resize(count)
		for i in count:
			var r := radius * sqrt(rng.randf())
			var a := rng.randf() * TAU
			points[i] = (up * ground_m + side * cos(a) * r + ahead * sin(a) * r).normalized() * ground_m
		return points

	for count in [100000, 400000, 1000000]:
		var multimesh := MultiMesh.new()
		multimesh.transform_format = MultiMesh.TRANSFORM_3D
		multimesh.mesh = plant
		multimesh.instance_count = count
		var points: PackedVector3Array = scatter.call(count, 150.0)
		for i in count:
			multimesh.set_instance_transform(i, Transform3D(upright, points[i]))
		var field := MultiMeshInstance3D.new()
		field.multimesh = multimesh
		add_child(field)
		await _measure("%d instanced plants within 150 m" % count)
		field.queue_free()

	var box := BoxMesh.new()
	box.size = Vector3(1.5, 2.5, 1.5)
	box.material = StandardMaterial3D.new()
	for count in [2000, 10000, 30000]:
		var holder := Node3D.new()
		add_child(holder)
		var points: PackedVector3Array = scatter.call(count, 400.0)
		for i in count:
			var thing := MeshInstance3D.new()
			thing.mesh = box
			holder.add_child(thing)
			thing.global_transform = Transform3D(upright, points[i] + up * 1.2)
		await _measure("%d separate objects within 400 m" % count)
		holder.queue_free()
	get_tree().quit()


# Frame times in the normal play loop at each simulation speed, standing still.
func _lag_check() -> void:
	_player.controls_enabled = false
	for i in 5:
		_sim.place(1, i)
	for speed in SPEEDS.size():
		_set_speed(speed)
		await _measure("simulation x%d" % SPEEDS[speed])
		print("STRESS   simulation actually ran at %d ticks/s; slowest refresh %.2f ms" % [
			_ticks_per_second, _refresh_cost_ms])
		_refresh_cost_ms = 0.0
	get_tree().quit()


func _measure(label: String) -> void:
	for i in 60:
		await get_tree().process_frame
	var frames := PackedFloat32Array()
	var last := Time.get_ticks_usec()
	var began := last
	while Time.get_ticks_usec() - began < 3000000:
		await get_tree().process_frame
		var now := Time.get_ticks_usec()
		frames.append((now - last) / 1000.0)
		last = now
	frames.sort()
	print("STRESS %s: %d fps, median frame %.1f ms, 99th percentile %.1f ms, worst %.1f ms, %d triangles, %d draw calls" % [
		label, frames.size() / 3, frames[frames.size() / 2], frames[int(frames.size() * 0.99)],
		frames[frames.size() - 1],
		Performance.get_monitor(Performance.RENDER_TOTAL_PRIMITIVES_IN_FRAME),
		Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME)])


func _shot(label: String) -> void:
	_refresh()
	# Let the terrain finish loading around the camera.
	var idle_since := Time.get_ticks_msec()
	var start := idle_since
	while Time.get_ticks_msec() - idle_since < 700 and Time.get_ticks_msec() - start < 60000:
		await get_tree().process_frame
		var tasks: Dictionary = VoxelEngine.get_stats()["tasks"]
		for key in ["streaming", "generation", "meshing", "main_thread"]:
			if int(tasks.get(key, 0)) > 0:
				idle_since = Time.get_ticks_msec()
	_refresh()
	var frames := 0
	var began := Time.get_ticks_msec()
	while Time.get_ticks_msec() - began < 1500:
		await get_tree().process_frame
		frames += 1
	var s: PackedFloat32Array = _sim.summary()
	print("DEMO %s: %d fps, phase %s, air %.2f, sea %.0f%%, life %.0f%%" % [
		label, frames / 1.5, PHASES[int(s[8])], s[1], s[4] * 100.0, s[6] * 100.0])
	if DisplayServer.get_name() == "headless":
		return
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path("res://shots"))
	await RenderingServer.frame_post_draw
	get_viewport().get_texture().get_image().save_png(
		ProjectSettings.globalize_path("res://shots/%s.png" % label))
