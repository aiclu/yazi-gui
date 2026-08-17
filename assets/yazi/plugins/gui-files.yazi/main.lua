local M = {}

--- 在目录切换/光标移动时，把当前目录的完整文件列表通过 DDS 发布出去，
--- 供外部 GUI（yazi-gui）订阅渲染。kind = "gui-files"。
local function publish_files()
	local current = cx.active.current
	local files = {}
	local n = #current.files
	for i = 1, n do
		local f = current.files[i]
		local cha = f.cha
		files[i] = {
			name = tostring(f.name),
			is_dir = not not cha.is_dir,
			is_hidden = not not cha.is_hidden,
			size = cha.len or 0,
			mtime = cha.mtime or 0,
		}
	end
	local hovered = current.hovered
	ps.pub("gui-files", {
		cwd = tostring(current.cwd),
		files = files,
		hovered = hovered and tostring(hovered.url) or nil,
	})
end

function M.setup()
	ps.sub("cd", publish_files)
	ps.sub("hover", publish_files)
end

return M
