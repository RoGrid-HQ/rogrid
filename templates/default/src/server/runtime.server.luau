local services = (script.Parent :: Instance):WaitForChild("services")

for _, module in services:GetChildren() do
	if module:IsA("ModuleScript") then
		local service = require(module)
		if service.Start then
			service:Start()
		end
	end
end