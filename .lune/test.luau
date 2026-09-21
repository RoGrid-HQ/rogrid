require("../packages/rogrid/tests/validate")
require("../packages/rogrid/tests/runtime")
require("../packages/rogrid/tests/requests")
local fs = require("@lune/fs")
local native = require("../packages/rogrid/tests/native")(require("@lune/roblox"))
local names = {}
for name in string.gmatch(fs.readFile("crates/rogrid/src/codegen/types.rs"), '=>%s*"([%w]+)"') do
    names[name] = true
    assert(native[name] ~= nil or name == "DateTime", `Add a test value for new payload type {name}`)
end
for name in native do assert(names[name], `Remove obsolete payload type test {name}`) end
