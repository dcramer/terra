extends Node3D

# Simulation toy (docs/prototypes.md, build 2): the planet simulation on a bare globe,
# with nothing else. Place machines and watch what they do.
#
#   Right-drag   orbit          Wheel   zoom
#   Left-click   place the selected machine
#   1 2 3        heater, gas generator, seeder
#   V            next view (surface, temperature, moisture, life)
#   Space        pause          - =     slower, faster
#
# For automated runs:  -- --scenario=hot|cool|cool_melt --ticks=8000   or   -- --bench

const SUBDIVISIONS := 5
const RADIUS_M := 2000.0
const MACHINES := ["Heater", "Gas generator", "Seeder"]
const MACHINE_COLOURS := [Color(1.0, 0.35, 0.1), Color(1.0, 0.9, 0.2), Color(0.3, 1.0, 0.3)]
const VIEWS := ["Surface", "Temperature", "Moisture", "Life"]
const PHASES := ["Dead", "Thawing", "Wet", "Green", "Breathable"]
const SPEEDS := [0, 1, 10, 100]

var _sim: RefCounted
var _mesh := ArrayMesh.new()
var _material := StandardMaterial3D.new()
var _triangles: PackedInt32Array
var _camera := Camera3D.new()
var _label := Label.new()
var _markers := Node3D.new()
var _yaw := 0.6
var _pitch := 0.45
var _distance := 2.3
var _tool := 0
var _view := 0
var _speed := 2
var _placed := [0, 0, 0]


func _ready() -> void:
	_sim = ClassDB.instantiate("TerraSim")
	_sim.setup(SUBDIVISIONS, 1)
	_triangles = _sim.triangles()

	_material.vertex_color_use_as_albedo = true
	_material.vertex_color_is_srgb = true
	_material.roughness = 1.0
	_material.cull_mode = BaseMaterial3D.CULL_DISABLED
	var globe := MeshInstance3D.new()
	globe.mesh = _mesh
	add_child(globe)
	add_child(_markers)

	var env := WorldEnvironment.new()
	env.environment = Environment.new()
	env.environment.background_mode = Environment.BG_COLOR
	env.environment.background_color = Color(0.02, 0.02, 0.04)
	env.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	env.environment.ambient_light_color = Color(0.45, 0.45, 0.5)
	add_child(env)
	# The light rides with the camera so the side being looked at is always lit.
	var light := DirectionalLight3D.new()
	light.rotation_degrees = Vector3(-20, 25, 0)
	_camera.add_child(light)
	add_child(_camera)

	var layer := CanvasLayer.new()
	_label.position = Vector2(16, 12)
	_label.add_theme_font_size_override("font_size", 18)
	layer.add_child(_label)
	add_child(layer)

	_update_camera()
	_refresh()
	_run_automation()


func _process(_delta: float) -> void:
	# The simulation runs on its own thread; this only sets its pace and redraws.
	_sim.set_rate(60 * SPEEDS[_speed])
	if SPEEDS[_speed] > 0:
		_refresh()


func _refresh() -> void:
	var vertices: PackedVector3Array = _sim.vertices(1.0 / RADIUS_M)
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = vertices
	# Close enough to a sphere that the positions serve as smooth normals.
	arrays[Mesh.ARRAY_NORMAL] = vertices
	arrays[Mesh.ARRAY_COLOR] = _sim.colours(_view)
	arrays[Mesh.ARRAY_INDEX] = _triangles
	_mesh.clear_surfaces()
	_mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	_mesh.surface_set_material(0, _material)

	var s: PackedFloat32Array = _sim.summary()
	_label.text = "%s   |   tool: %s   view: %s   speed: x%d\n" % [
			PHASES[int(s[8])], MACHINES[_tool], VIEWS[_view], SPEEDS[_speed]] \
		+ "tick %d   air %.2f   mean temperature %.0f C   sea %.0f%%   ice %.0f%%   life %.0f%%   oxygen %.0f%%\n" % [
			s[0], s[1], s[2], s[4] * 100.0, s[5] * 100.0, s[6] * 100.0, s[7] * 100.0] \
		+ "placed: %d heaters, %d gas generators, %d seeders" % _placed


func _place(machine: int, direction: Vector3) -> void:
	var cell: int = _sim.cell_at(direction)
	_sim.place(machine, cell)
	_placed[machine] += 1
	var marker := MeshInstance3D.new()
	var dot := SphereMesh.new()
	dot.radius = 0.012
	dot.height = 0.024
	var material := StandardMaterial3D.new()
	material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	material.albedo_color = MACHINE_COLOURS[machine]
	dot.material = material
	marker.mesh = dot
	marker.position = _sim.cell_direction(cell) * 1.09
	_markers.add_child(marker)


func _update_camera() -> void:
	_pitch = clampf(_pitch, -1.5, 1.5)
	_distance = clampf(_distance, 1.3, 8.0)
	_camera.position = Vector3(
		cos(_pitch) * sin(_yaw), sin(_pitch), cos(_pitch) * cos(_yaw)) * _distance
	_camera.look_at(Vector3.ZERO, Vector3.UP)


func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseMotion and event.button_mask & MOUSE_BUTTON_MASK_RIGHT:
		_yaw -= event.relative.x * 0.005
		_pitch += event.relative.y * 0.005
		_update_camera()
	elif event is InputEventMouseButton and event.pressed:
		if event.button_index == MOUSE_BUTTON_WHEEL_UP:
			_distance *= 0.9
			_update_camera()
		elif event.button_index == MOUSE_BUTTON_WHEEL_DOWN:
			_distance *= 1.1
			_update_camera()
		elif event.button_index == MOUSE_BUTTON_LEFT:
			var hit = _globe_hit(event.position)
			if hit != null:
				_place(_tool, hit)
				_refresh()
	elif event is InputEventKey and event.pressed and not event.echo:
		match event.keycode:
			KEY_1, KEY_2, KEY_3:
				_tool = event.keycode - KEY_1
			KEY_V:
				_view = (_view + 1) % VIEWS.size()
			KEY_SPACE:
				_speed = 0 if _speed > 0 else 2
			KEY_MINUS:
				_speed = maxi(_speed - 1, 0)
			KEY_EQUAL:
				_speed = mini(_speed + 1, SPEEDS.size() - 1)
		_refresh()


# Where a screen point hits the globe, as a direction from its centre, or null.
func _globe_hit(screen: Vector2) -> Variant:
	var origin := _camera.project_ray_origin(screen)
	var dir := _camera.project_ray_normal(screen)
	var b := origin.dot(dir)
	var disc := b * b - (origin.length_squared() - 1.0)
	if disc < 0.0:
		return null
	return (origin + dir * (-b - sqrt(disc))).normalized()


func _direction(latitude_deg: float, longitude_deg: float) -> Vector3:
	var lat := deg_to_rad(latitude_deg)
	var lon := deg_to_rad(longitude_deg)
	return Vector3(cos(lat) * sin(lon), sin(lat), cos(lat) * cos(lon))


# Scripted runs, so strategies can be compared and performance measured without a player.
func _run_automation() -> void:
	var scenario := ""
	var ticks := 8000
	var bench := false
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--scenario="):
			scenario = arg.get_slice("=", 1)
		elif arg.begins_with("--ticks="):
			ticks = int(arg.get_slice("=", 1))
		elif arg == "--bench":
			bench = true
	if scenario == "" and not bench:
		return

	_speed = 0
	var gas := 5 if scenario == "hot" else 2
	for i in gas:
		_place(1, _direction(0, 200 + i * 8))
	for lon in range(0, 360, 45):
		_place(2, _direction(20, lon))
		_place(2, _direction(-20, lon + 22))
		_place(2, _direction(45, lon + 10))
	if scenario == "cool_melt":
		for pole in [1.0, -1.0]:
			_place(0, _direction(88 * pole, 0))
			for lon in range(0, 360, 45):
				_place(0, _direction(78 * pole, lon))
			for lon in range(0, 360, 30):
				_place(0, _direction(68 * pole, lon))

	if bench:
		_speed = 2
		var frames := PackedFloat32Array()
		var last := Time.get_ticks_usec()
		for i in 600:
			await get_tree().process_frame
			var now := Time.get_ticks_usec()
			frames.append((now - last) / 1000.0)
			last = now
		frames.sort()
		print("TOY bench at x%d: median frame %.2f ms, 99th percentile %.2f ms, adapter %s" % [
			SPEEDS[_speed], frames[frames.size() / 2], frames[int(frames.size() * 0.99)],
			RenderingServer.get_video_adapter_name()])
		get_tree().quit()
		return

	var start := Time.get_ticks_msec()
	_sim.step(ticks)
	var elapsed := Time.get_ticks_msec() - start
	var s: PackedFloat32Array = _sim.summary()
	print("TOY %s after %d ticks (%d ms): phase=%s air=%.2f temp=%.1f sea=%.0f%% ice=%.0f%% life=%.0f%% oxygen=%.1f%%" % [
		scenario, ticks, elapsed, PHASES[int(s[8])], s[1], s[2], s[4] * 100.0, s[5] * 100.0,
		s[6] * 100.0, s[7] * 100.0])
	if DisplayServer.get_name() == "headless":
		get_tree().quit()
		return
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path("res://shots"))
	for view in VIEWS.size():
		_view = view
		_refresh()
		await get_tree().process_frame
		await get_tree().process_frame
		await RenderingServer.frame_post_draw
		get_viewport().get_texture().get_image().save_png(
			ProjectSettings.globalize_path("res://shots/%s_%s.png" % [scenario, VIEWS[view].to_lower()]))
	get_tree().quit()
