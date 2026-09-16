--!strict
local Hello = {}

function Hello.greet(name: string): string
	return `Hello, {name}!`
end

return Hello
