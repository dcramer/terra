extends Node

# Spike check for question 3 in docs/prototypes.md: what a bare planet looks like and
# costs from orbit down to the ground, and whether digging causes a visible hitch.
# Needs a real GPU. Saves screenshots to shots/ and prints timings.
#
#   -- --radius=2000

const Planet = preload("res://planet.gd")

var _radius := 2000.0
var _cam: Camera3D
var _tool: VoxelTool


func _ready() -> void:
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--radius="):
			_radius = float(arg.get_slice("=", 1))

	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-35, 30, 0)
	sun.shadow_enabled = true
	sun.directional_shadow_max_distance = 600.0
	add_child(sun)
	var env := WorldEnvironment.new()
	env.environment = Environment.new()
	env.environment.background_mode = Environment.BG_COLOR
	env.environment.background_color = Color(0.02, 0.02, 0.04)
	env.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	env.environment.ambient_light_color = Color(0.25, 0.27, 0.32)
	add_child(env)

	var terrain := Planet.make_terrain(_radius)
	var material := StandardMaterial3D.new()
	material.albedo_color = Color(0.62, 0.5, 0.4)
	terrain.material = material
	add_child(terrain)
	_tool = terrain.get_voxel_tool()
	_tool.channel = VoxelBuffer.CHANNEL_SDF
	_tool.mode = VoxelTool.MODE_REMOVE
	# Collision meshes near players, as a real game would need.
	terrain.generate_collisions = true
	terrain.collision_lod_count = 2

	# Stand-ins for three other co-op players elsewhere on the planet. The host has to
	# keep terrain data and collision loaded around each of them.
	for dir in [Vector3.LEFT, Vector3.FORWARD, Vector3.DOWN]:
		var other := VoxelViewer.new()
		other.requires_visuals = false
		other.position = dir * _radius
		add_child(other)

	_cam = Camera3D.new()
	_cam.near = 0.2
	_cam.far = _radius * 20.0
	var viewer := VoxelViewer.new()
	viewer.view_distance = int(_radius * 10.0)
	_cam.add_child(viewer)
	add_child(_cam)
	_cam.make_current()

	# Stand on the lit side, a little off noon so the terrain has shading.
	var up := (sun.global_basis.z + sun.global_basis.x * 0.5).normalized()
	var tangent := up.cross(Vector3(0.3, 1.0, 0.2)).normalized()

	await _stop("1_orbit", up * _radius * 2.5, Vector3.ZERO, tangent)
	var ground := up * Planet.surface_distance(_tool, up, _radius)
	var eye := ground + up * 3.0
	await _fly("descent orbit to ground in 10 s", up * _radius * 2.5, ground + up * 30.0, 10.0, false)
	await _stop("2_high", up * _radius * 1.15, up * _radius + tangent * _radius * 0.6, up)
	await _stop("3_ground", eye, eye + tangent * 50.0 - up * 4.0, up)

	# Dig a crater in view and watch frame times while the terrain remeshes.
	var ahead := (ground + tangent * 16.0).normalized()
	var site := ahead * Planet.surface_distance(_tool, ahead, _radius)
	var start := Time.get_ticks_usec()
	_tool.do_sphere(site, 5.0)
	var dig_ms := (Time.get_ticks_usec() - start) / 1000.0
	var worst := 0.0
	var last := Time.get_ticks_usec()
	for i in 240:
		await get_tree().process_frame
		var now := Time.get_ticks_usec()
		worst = maxf(worst, (now - last) / 1000.0)
		last = now
	print("VIEW dig call=%.2f ms, worst frame in the next 240 frames=%.2f ms" % [dig_ms, worst])
	await _stop("4_after_dig", eye, eye + tangent * 50.0 - up * 4.0, up)

	# Low flight over the surface at 100 m/s, then 400 m/s.
	var low := ground + up * 40.0
	for speed in [100.0, 400.0]:
		var angle: float = speed * 10.0 / _radius
		var to := low.rotated(up.cross(tangent).normalized(), angle)
		await _fly("surface flight at %d m/s for 10 s" % speed, low, to, 10.0, true)
	get_tree().quit()


# Moves the camera along a path and reports frame times, to catch hitches from terrain
# loading. Along the surface it follows the curve of the planet.
func _fly(label: String, from: Vector3, to: Vector3, seconds: float, along_surface: bool) -> void:
	var frames := PackedFloat32Array()
	var max_pending := 0
	var start := Time.get_ticks_usec()
	var last := start
	while true:
		var t := (Time.get_ticks_usec() - start) / 1000000.0 / seconds
		if t >= 1.0:
			break
		if along_surface:
			var pos := from.slerp(to, t)
			var up := pos.normalized()
			var forward := (to - from).slide(up).normalized()
			_cam.global_position = pos
			_cam.look_at(pos + forward * 60.0 - up * 15.0, up)
		else:
			_cam.global_position = from.lerp(to, t)
			_cam.look_at(Vector3.ZERO, Vector3(0.3, 1.0, 0.2))
		await get_tree().process_frame
		var now := Time.get_ticks_usec()
		frames.append((now - last) / 1000.0)
		last = now
		max_pending = maxi(max_pending, _pending_tasks())
	frames.sort()
	print("VIEW fly %s: avg fps=%d, 99th percentile frame=%.1f ms, worst frame=%.1f ms, most queued terrain tasks=%d" % [
		label, int(frames.size() / seconds), frames[int(frames.size() * 0.99)],
		frames[frames.size() - 1], max_pending])
	var catch_up := await _wait_until_loaded()
	print("VIEW fly %s: terrain caught up %.1f s after stopping" % [label, catch_up])


func _pending_tasks() -> int:
	var tasks: Dictionary = VoxelEngine.get_stats()["tasks"]
	var pending := 0
	for key in ["streaming", "generation", "meshing", "main_thread"]:
		pending += int(tasks.get(key, 0))
	return pending


func _stop(label: String, pos: Vector3, target: Vector3, up: Vector3) -> void:
	_cam.global_position = pos
	_cam.look_at(target, up)
	var load_s := await _wait_until_loaded()
	var frames := 0
	var start := Time.get_ticks_msec()
	while Time.get_ticks_msec() - start < 2000:
		await get_tree().process_frame
		frames += 1
	print("VIEW %s radius=%d load=%.1fs fps=%d triangles=%d draw_calls=%d memory=%dMB" % [
		label, _radius, load_s, frames / 2,
		Performance.get_monitor(Performance.RENDER_TOTAL_PRIMITIVES_IN_FRAME),
		Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME),
		OS.get_static_memory_usage() / 1048576])
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path("res://shots"))
	get_viewport().get_texture().get_image().save_png(
		ProjectSettings.globalize_path("res://shots/%d_%s.png" % [_radius, label]))


# Waits until the voxel engine has had no pending work for half a second.
func _wait_until_loaded() -> float:
	var start := Time.get_ticks_msec()
	var idle_since := start
	while Time.get_ticks_msec() - idle_since < 500 and Time.get_ticks_msec() - start < 90000:
		await get_tree().process_frame
		var tasks: Dictionary = VoxelEngine.get_stats()["tasks"]
		for key in ["streaming", "generation", "meshing", "main_thread"]:
			if int(tasks.get(key, 0)) > 0:
				idle_since = Time.get_ticks_msec()
	return (idle_since - start) / 1000.0
