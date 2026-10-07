extends Node

# Spike test for question 1 in docs/prototypes.md: can several peers dig the same
# planet and end up with identical voxel data, including a peer that joins late?
#
# The host is authoritative. Digs are sent as operations (centre + radius), never as
# voxel data: every peer generates the same planet and applies the same operations.
#
#   --host            start a server and run the checks
#   --join            connect to the host
#   --join --dig      connect, then request one dig

const Planet = preload("res://planet.gd")

const PORT := 24565
const DIG_RADIUS := 6.0
const HOST_DIG_DIR := Vector3.UP
const CLIENT_DIG_DIR := Vector3(0.05, 1.0, 0.02)
const CHECK_SIZE := 64

var _terrain: VoxelLodTerrain
var _tool: VoxelTool
var _ops: Array = []
var _reports := {}
var _check_origin: Vector3i


func _ready() -> void:
	_terrain = Planet.make_terrain(Planet.RADIUS)
	add_child(_terrain)
	var viewer := VoxelViewer.new()
	viewer.position = HOST_DIG_DIR * Planet.RADIUS
	add_child(viewer)
	_tool = _terrain.get_voxel_tool()
	_tool.channel = VoxelBuffer.CHANNEL_SDF
	_tool.mode = VoxelTool.MODE_REMOVE
	await get_tree().create_timer(1.0).timeout
	# Every peer derives the same check box from the same untouched planet.
	var ground := _ground_point(HOST_DIG_DIR)
	_check_origin = Vector3i(ground) - Vector3i.ONE * (CHECK_SIZE / 2)

	var args := OS.get_cmdline_user_args()
	var peer := ENetMultiplayerPeer.new()
	if args.has("--host"):
		peer.create_server(PORT)
		multiplayer.multiplayer_peer = peer
		_run_host_checks()
	else:
		peer.create_client("127.0.0.1", PORT)
		multiplayer.multiplayer_peer = peer
		if args.has("--dig"):
			await multiplayer.connected_to_server
			await get_tree().create_timer(1.0).timeout
			request_dig.rpc_id(1, _ground_point(CLIENT_DIG_DIR), DIG_RADIUS)


func _run_host_checks() -> void:
	multiplayer.peer_connected.connect(func(id): replay_log.rpc_id(id, _ops))
	var ok := true

	await _wait_for_peers(1)
	var before := _region_hash()
	var solid_before := _solid_count()
	_host_dig(_ground_point(HOST_DIG_DIR), DIG_RADIUS)
	while _ops.size() < 2:
		await get_tree().process_frame
	await get_tree().create_timer(0.5).timeout
	var removed := solid_before - _solid_count()
	print("dig points: ", _ops.map(func(op): return op[0]), " solid voxels removed: ", removed)
	ok = _check("digging changed the host's voxel data", before != _region_hash()) and ok
	ok = _check("two digs removed solid ground (%d voxels)" % removed, removed > 500) and ok
	ok = await _compare("live sync: client matches host after one dig each") and ok

	await _wait_for_peers(2)
	ok = await _compare("late join: new client matches host after replaying the log") and ok

	print("RESULT ", "PASS" if ok else "FAIL")
	finish.rpc()
	await get_tree().create_timer(0.5).timeout
	get_tree().quit(0 if ok else 1)


func _host_dig(center: Vector3, radius: float) -> void:
	_ops.append([center, radius])
	apply_dig.rpc(center, radius)


func _compare(label: String) -> bool:
	await get_tree().create_timer(0.5).timeout
	_reports.clear()
	request_hash.rpc()
	var peers := multiplayer.get_peers()
	while _reports.size() < peers.size():
		await get_tree().process_frame
	var mine := _region_hash()
	var same := true
	for id in _reports:
		same = same and _reports[id] == mine
	return _check("%s (%d peers, hash %s)" % [label, peers.size(), mine.left(12)], same)


func _check(label: String, passed: bool) -> bool:
	print("PASS " if passed else "FAIL ", label)
	return passed


func _wait_for_peers(count: int) -> void:
	while multiplayer.get_peers().size() < count:
		await get_tree().process_frame


func _ground_point(dir: Vector3) -> Vector3:
	var n := dir.normalized()
	return n * Planet.surface_distance(_tool, n, Planet.RADIUS)


func _solid_count() -> int:
	var count := 0
	for x in CHECK_SIZE:
		for y in CHECK_SIZE:
			for z in CHECK_SIZE:
				if _tool.get_voxel_f(_check_origin + Vector3i(x, y, z)) < 0.0:
					count += 1
	return count


# Hash of the raw signed-distance values in a box around the dig sites.
func _region_hash() -> String:
	var buffer := VoxelBuffer.new()
	buffer.create(CHECK_SIZE, CHECK_SIZE, CHECK_SIZE)
	_tool.copy(_check_origin, buffer, 1 << VoxelBuffer.CHANNEL_SDF)
	buffer.decompress_channel(VoxelBuffer.CHANNEL_SDF)
	var ctx := HashingContext.new()
	ctx.start(HashingContext.HASH_SHA256)
	ctx.update(buffer.get_channel_as_byte_array(VoxelBuffer.CHANNEL_SDF))
	return ctx.finish().hex_encode()


@rpc("any_peer", "call_remote", "reliable")
func request_dig(center: Vector3, radius: float) -> void:
	if multiplayer.is_server():
		_host_dig(center, radius)


@rpc("authority", "call_local", "reliable")
func apply_dig(center: Vector3, radius: float) -> void:
	_tool.do_sphere(center, radius)


@rpc("authority", "call_remote", "reliable")
func replay_log(ops: Array) -> void:
	for op in ops:
		_tool.do_sphere(op[0], op[1])


@rpc("authority", "call_remote", "reliable")
func request_hash() -> void:
	report_hash.rpc_id(1, _region_hash())


@rpc("any_peer", "call_remote", "reliable")
func report_hash(value: String) -> void:
	_reports[multiplayer.get_remote_sender_id()] = value


@rpc("authority", "call_remote", "reliable")
func finish() -> void:
	get_tree().quit()
