local controllers = (script.Parent :: Instance):WaitForChild("controllers")

for _, module in controllers:GetChildren() do
	if module:IsA("ModuleScript") then
		local controller = require(module)
		if controller.Start then
			controller:Start()
		end
	end
end