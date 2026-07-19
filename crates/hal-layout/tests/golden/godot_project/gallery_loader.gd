extends SceneTree

func _init():
	var scene = load("res://control_gallery.tscn").instantiate()
	root.add_child(scene)

	# 强制 Root 尺寸
	if scene is Control:
		scene.size = Vector2i(960, 640)
		scene.set_anchors_preset(Control.PRESET_FULL_RECT)

	await process_frame
	await process_frame

	var result = {}
	_collect_layout(scene, "", result)

	var json = JSON.stringify(result, "  ")
	print("=== GOLDEN_START ===")
	print(json)
	print("=== GOLDEN_END ===")

	var file = FileAccess.open("user://gallery_golden.json", FileAccess.WRITE)
	file.store_string(json)
	file.close()

	quit()

func _collect_layout(node: Node, path: String, result: Dictionary):
	if node is Control:
		var control = node as Control
		var rect = control.get_global_rect()
		result[path] = {
			"name": node.name,
			"x": rect.position.x,
			"y": rect.position.y,
			"width": rect.size.x,
			"height": rect.size.y,
		}
	for child in node.get_children():
		var child_path = path + "/" + child.name if path != "" else child.name
		_collect_layout(child, child_path, result)
