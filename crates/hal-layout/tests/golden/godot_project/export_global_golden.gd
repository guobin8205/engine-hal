extends SceneTree

# 通用全局坐标 golden 导出器。
#
# 用法：
#   godot --headless --script export_global_golden.gd -- <scene_file> <out_json> [window_w] [window_h]
#
# 导出格式（全局坐标，左上角原点 Y 向下，和 Godot Control.global_position 一致）：
#   { "<path>": {"name": "...", "gx": 0, "gy": 0, "w": 100, "h": 50}, ... }
#
# path 从场景根名开始（如 "ControlGallery/MainPanel"），和 hal-layout FlatNode.path 对齐。

var _frame_count = 0
var _scene: Node = null
var _done = false
var _scene_path := ""
var _out_path := ""
var _window := Vector2i(960, 640)

func _init():
	var args = OS.get_cmdline_user_args()
	# args 是 -- 之后的参数
	if args.size() >= 2:
		_scene_path = args[0]
		_out_path = args[1]
	if args.size() >= 4:
		_window = Vector2i(int(args[2]), int(args[3]))

	if _scene_path == "":
		print("ERROR: 用法 godot --headless --script export_global_golden.gd -- <scene> <out_json> [w] [h]")
		quit(1)
		return

	print("加载场景: ", _scene_path)
	_scene = load(_scene_path).instantiate()
	root.add_child(_scene)
	if _scene is Control:
		_scene.set_deferred("size", _window)

func _process(delta):
	if _done:
		return
	_frame_count += 1
	if _frame_count == 3:
		if _scene is Control:
			_scene.size = _window
	if _frame_count >= 5:
		_done = true
		_export()
		quit()

func _export():
	var result = {}
	_collect(_scene, "", result)

	var json = JSON.stringify(result, "  ")
	var file = FileAccess.open(_out_path, FileAccess.WRITE)
	if file == null:
		print("ERROR: 无法写 ", _out_path)
		quit(1)
		return
	file.store_string(json)
	file.close()

	print("导出: ", _out_path, " (", result.size(), " 节点)")
	print("窗口: ", _window)

func _collect(node: Node, path: String, result: Dictionary):
	if node is Control:
		var control = node as Control
		var child_path = path
		if path == "":
			child_path = node.name
		else:
			child_path = path + "/" + node.name
		var gp = control.global_position
		var s = control.size
		result[child_path] = {
			"name": node.name,
			"gx": gp.x,
			"gy": gp.y,
			"w": s.x,
			"h": s.y,
		}
		for child in node.get_children():
			_collect(child, child_path, result)
	else:
		# 非 Control 节点（如 Node2D），透传给子节点但不记录
		for child in node.get_children():
			_collect(child, path, result)
