local RoGrid = {}

-- Loads every ModuleScript in `folder`, then runs two phases:
--   1. Init()  on all modules. Nothing is running yet, so modules can
--              safely grab references to each other here.
--   2. Start() on all modules. Everything is initialised; go.
-- Modules without Init or Start are fine.
function RoGrid.Start(folder: Instance)
	local modules = {}
	for _, child in folder:GetChildren() do
		if child:IsA("ModuleScript") then
			table.insert(modules, require(child))
		end
	end
	for _, m in modules do
		if m.Init then m:Init() end
	end
	for _, m in modules do
		if m.Start then m:Start() end
	end
end

return RoGrid