extends Control

# Golden test 导出脚本
# 场景加载后，等帧让布局完成，然后导出所有 Control 的最终 position+size

func _ready():
	# 强制设置 Root Control 的 size 为标准测试分辨率
	# headless 模式下 window.size 是 (64,64)，Control 用的是 dummy display 的默认值
	# 为了和 hal-layout/Cocos 的 960x640 一致，手动覆盖
	self.size = Vector2i(960, 640)
	self.set_anchors_preset(Control.PRESET_FULL_RECT)

	await get_tree().process_frame
	await get_tree().process_frame

	var result = {}
	_collect_layout(self, "", result)

	var json = JSON.stringify(result, "  ")
	print("=== GOLDEN_START ===")
	print(json)
	print("=== GOLDEN_END ===")

	var file = FileAccess.open("user://layout_golden.json", FileAccess.WRITE)
	file.store_string(json)
	file.close()

	get_tree().quit()

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
