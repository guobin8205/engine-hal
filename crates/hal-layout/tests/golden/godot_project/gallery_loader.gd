extends SceneTree

var _frame_count = 0
var _scene: Node = null
var _done = false

func _init():
	_scene = load("res://control_gallery.tscn").instantiate()
	root.add_child(_scene)

	if _scene is Control:
		_scene.set_deferred("size", Vector2i(960, 640))

func _process(delta):
	if _done:
		return
	_frame_count += 1

	# 等 5 帧让布局稳定
	if _frame_count == 3:
		if _scene is Control:
			_scene.size = Vector2i(960, 640)

	if _frame_count >= 5:
		_done = true
		_export()

func _export():
	var result = {}
	_collect_layout(_scene, "", result)

	var json = JSON.stringify(result, "  ")
	print("=== GOLDEN_START ===")
	print(json)
	print("=== GOLDEN_END ===")

	var file = FileAccess.open("user://gallery_golden.json", FileAccess.WRITE)
	file.store_string(json)
	file.close()

	print("=== Root size: ", _scene.size, " ===")
	print("=== Nodes: ", result.size(), " ===")

	quit()

func _collect_layout(node: Node, path: String, result: Dictionary):
	if node is Control:
		var control = node as Control
		# 用 get_rect() 但修正 grow_direction 导致的偏移
		# get_rect() 返回 data.pos + data.offset，包含 anchor 和 grow 的综合效果
		# 这是 Control 在父节点空间中的实际矩形
		var rect = control.get_rect()
		var min_size = control.get_combined_minimum_size()
		result[path] = {
			"name": node.name,
			"x": rect.position.x,
			"y": rect.position.y,
			"width": rect.size.x,
			"height": rect.size.y,
			"min_width": min_size.x,
			"min_height": min_size.y,
		}
	for child in node.get_children():
		var child_path = path + "/" + child.name if path != "" else child.name
		_collect_layout(child, child_path, result)
