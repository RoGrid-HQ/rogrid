local roblox = require("@lune/roblox")
local validate = require("../src/validate")
local count = 0
local function test(name, shape, value, expected)
    local message = validate({ { name = "payload", shape = shape } }, value)
    if expected then
        assert(message and string.find(message, expected, 1, true), `{name}: expected {expected}, got {message}`)
    else
        assert(message == nil, `{name}: {message}`)
    end
    count += 1
end

for _, case in {
    { "string", "hello" }, { "number", 42 }, { "boolean", false }, { "buffer", buffer.create(8) },
} do
    test(case[1], case[1], case[2])
    test(case[1] .. " wrong type", case[1], {}, "expected")
end
test("NaN", "number", 0/0, "invalid")
test("infinity", "number", math.huge, "invalid")
test("negative infinity", "number", -math.huge, "invalid")
test("string boundary", "string", string.rep("a", 4096))
test("long string", "string", string.rep("a", 4097), "oversized")
test("buffer boundary", "buffer", buffer.create(65536))
test("long buffer", "buffer", buffer.create(65537), "oversized")
test("nil", "nil", nil)
test("optional absent", { optional = "string" }, nil)
test("optional present", { optional = "string" }, "hello")
test("optional wrong", { optional = "string" }, false, "expected")
test("false literal", { literal = false }, false)
test("false isn't nil", { literal = false }, nil, "literal")
test("false isn't true", { literal = false }, true, "literal")
test("string literal", { literal = "equip" }, "equip")
test("wrong literal", { literal = "equip" }, "drop", "literal")
local record = { record = { id = "string", count = "number", note = { optional = "string" } } }
test("record", record, { id = "a", count = 1 })
test("optional field", record, { id = "a", count = 1, note = "hi" })
test("bad optional field", record, { id = "a", count = 1, note = 3 }, "payload.note")
test("unknown field", record, { id = "a", count = 1, cachedIcon = {} }, "payload.cachedIcon: unknown field")
test("missing field", record, { id = "a" }, "payload.count")
test("numeric record key", record, { id = "a", count = 1, [1] = 3 }, "unknown field")
test("metatable", record, setmetatable({}, { __index = { id = "a", count = 1 } }), "metatables")
local array = { array = "number" }
test("array", array, { 1, 2, 3 })
test("empty array", array, {})
test("array wrong value", array, { 1, "2" }, "payload[2]")
for name, value in {
    hole = { [1] = 1, [3] = 3 }, mixed = { 1, x = 2 }, zero = { [0] = 1 },
    negative = { [-1] = 1 }, fraction = { [1.5] = 1 }, distant = { [1000000000] = 1 },
} do
    test(name, array, value, "payload")
end
local dictionary = { dictionary = { array = "number" } }
test("nested dictionary", dictionary, { a = { 1 }, b = {} })
test("bad dictionary value", dictionary, { a = { false } }, "payload.a[1]")
test("numeric dictionary key", dictionary, { [1] = {} }, "string keys")
test("long dictionary key", dictionary, { [string.rep("a",4097)] = {} }, "string keys")
test("union string", { union = { "string", "number" } }, "a")
test("union number", { union = { "string", "number" } }, 1)
test("union mismatch", { union = { "string", "number" } }, {}, "no union branch")
local union = { union = {
    { record = { kind = { literal = "a" }, value = "string" } },
    { record = { kind = { literal = "b" }, value = "number" } },
} }
test("tagged union first", union, { kind = "a", value = "hi" })
test("tagged union second", union, { kind = "b", value = 1 })
test("tagged union wrong", union, { kind = "b", value = "hi" }, "no union branch")
local cycle = {}; cycle[1] = cycle
test("cycle", { array = { array = "number" } }, cycle, "cyclic")
local shared = { 1 }
test("shared table isn't a cycle", { array = array }, { shared, shared })
test("array work limit", array, table.create(4096, 1), "work limit")
test("wide record limit", { record = {} }, table.create(10000, 1), "unknown field")
local big = {}; for i = 1, 4096 do big[tostring(i)] = 1 end
test("dictionary work limit", { dictionary = "number" }, big, "work limit")
local branches = {}; for i = 1, 4096 do branches[i] = "number" end
branches[4097] = "string"
test("union budget isn't refunded", { union = branches }, "match at end", "work limit")

local params = { { name = "first", shape = "number" }, { name = "second", shape = { optional = "string" } } }
assert(validate(params, 1) == nil)
assert(validate(params, 1, nil) == nil)
assert(validate(params, 1, "hi") == nil)
assert(string.find(validate(params), "first", 1, true))
assert(string.find(validate(params, 1, nil, nil), "too many", 1, true))
assert(validate({ { name = "a", shape = { optional = "number" } }, { name = "b", shape = "boolean" } }, nil, false) == nil)
assert(validate({ { name = "a", shape = { optional = "number" } }, { name = "b", shape = "boolean" } }, nil) ~= nil)
assert(validate({}) == nil and validate({}, nil) ~= nil)
assert(validate({ { name="a",shape=array }, {name="b",shape=array} },table.create(1100,1),table.create(1100,1)) ~= nil)
count += 9

local V2, V3, C = roblox.Vector2, roblox.Vector3, roblox.Color3
local native = require("./native")(roblox)
for name, value in native do
    test(name, name, value)
    test(name .. " wrong type", name, {}, "expected")
end
test("Vector3 NaN", "Vector3", V3.new(0/0,0,0), "invalid")
test("Vector2 infinity", "Vector2", V2.new(0,math.huge), "invalid")
test("Color3 NaN", "Color3", C.new(0,0/0,0), "invalid")
test("CFrame NaN", "CFrame", roblox.CFrame.new(0/0,0,0), "invalid")
test("UDim infinity", "UDim", roblox.UDim.new(math.huge,0), "invalid")
test("Ray NaN", "Ray", roblox.Ray.new(V3.zero,V3.new(0/0,0,0)), "invalid")
test("UDim2 infinity", "UDim2", roblox.UDim2.new(0,0,math.huge,0), "invalid")
test("Rect infinity", "Rect", roblox.Rect.new(V2.new(-math.huge,0),V2.one), "invalid")
test("NumberRange infinity", "NumberRange", roblox.NumberRange.new(0,math.huge), "invalid")
test("NumberKeypoint NaN", "NumberSequenceKeypoint", roblox.NumberSequenceKeypoint.new(0,0/0), "invalid")
test("ColorKeypoint NaN", "ColorSequenceKeypoint", roblox.ColorSequenceKeypoint.new(0,C.new(0/0,0,0)), "invalid")
test("NumberSequence NaN", "NumberSequence", roblox.NumberSequence.new({roblox.NumberSequenceKeypoint.new(0,0/0),roblox.NumberSequenceKeypoint.new(1,1)}), "invalid")
test("ColorSequence NaN", "ColorSequence", roblox.ColorSequence.new({roblox.ColorSequenceKeypoint.new(0,C.new(0/0,0,0)),roblox.ColorSequenceKeypoint.new(1,C.new())}), "invalid")
test("enum family", { enum = "Material" }, roblox.Enum.Material.Plastic)
test("wrong enum family", { enum = "Material" }, roblox.Enum.Font.Arial, "Enum.Material")
local part = roblox.Instance.new("Part")
test("Instance", { instance = "Instance" }, part)
test("subclass", { instance = "BasePart" }, part)
test("wrong class", { instance = "Model" }, part, "Model")
test("non-instance", { instance = "Instance" }, {}, "Instance")
test("missing instance", { instance = "Instance" }, nil, "Instance")
test("optional missing instance", { optional = { instance = "Instance" } }, nil)
print(`Validator: {count} checks passed`)
