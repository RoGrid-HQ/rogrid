--!strict
-- Generated descriptors are trusted. Received values are not.
export type Parameter = { name: string, shape: any }

local function finite(n: number): boolean
    return n == n and math.abs(n) < math.huge
end

local function vector2(v: any): boolean
    return finite(v.X) and finite(v.Y)
end

local function vector3(v: any): boolean
    return vector2(v) and finite(v.Z)
end

local function color(v: any): boolean
    return finite(v.R) and finite(v.G) and finite(v.B)
end

local function cframe(v: CFrame): boolean
    for _, n in { v:GetComponents() } do
        if not finite(n) then return false end
    end
    return true
end

local function udim(v: UDim): boolean
    return finite(v.Scale) and finite(v.Offset)
end

local function numberKeypoint(v: NumberSequenceKeypoint): boolean
    return finite(v.Time) and finite(v.Value) and finite(v.Envelope)
end

local function colorKeypoint(v: ColorSequenceKeypoint): boolean
    return finite(v.Time) and color(v.Value)
end

local function sequence(v: any, check: (any) -> boolean): boolean
    local points = v.Keypoints
    for _, point in points do
        if not check(point) then return false end
    end
    return true
end

local function valid(): boolean return true end

-- To add a native value type, add its name in codegen/types.rs and its check here.
-- typeof is checked before these functions run. Integers and enums cannot hold NaN.
local leaves: { [string]: (any) -> boolean } = {
    ["nil"] = valid,
    string = valid,
    number = finite,
    boolean = valid,
    buffer = valid,
    Vector2 = vector2,
    Vector3 = vector3,
    Vector2int16 = valid,
    Vector3int16 = valid,
    CFrame = cframe,
    Color3 = color,
    BrickColor = valid,
    UDim = udim,
    UDim2 = function(v) return udim(v.X) and udim(v.Y) end,
    Rect = function(v) return vector2(v.Min) and vector2(v.Max) end,
    Ray = function(v) return vector3(v.Origin) and vector3(v.Direction) end,
    Region3 = function(v) return cframe(v.CFrame) and vector3(v.Size) end,
    Region3int16 = valid,
    NumberRange = function(v) return finite(v.Min) and finite(v.Max) end,
    NumberSequenceKeypoint = numberKeypoint,
    ColorSequenceKeypoint = colorKeypoint,
    NumberSequence = function(v) return sequence(v, numberKeypoint) end,
    ColorSequence = function(v) return sequence(v, colorKeypoint) end,
    DateTime = function(v) return finite(v.UnixTimestampMillis) end,
    Axes = valid,
    Faces = valid,
    PhysicalProperties = function(v)
        return finite(v.Density) and finite(v.Friction) and finite(v.Elasticity)
            and finite(v.FrictionWeight) and finite(v.ElasticityWeight)
            and finite(v.AcousticAbsorption)
    end,
    Font = valid,
    EnumItem = valid,
}

local function fieldPath(path: string, key: any): string
    if typeof(key) == "string" then
        return path .. "." .. key
    end
    if typeof(key) == "number" then return path .. "[" .. tostring(key) .. "]" end
    return path .. "[invalid key]"
end

local function check(shape: any, value: any, active: { [any]: boolean }, path: string): string?
    if typeof(shape) == "string" then
        if typeof(value) ~= shape then return path .. ": expected " .. shape end
        local leaf = leaves[shape]
        if not leaf or not leaf(value) then return path .. ": invalid " .. shape end
        return nil
    end
    if shape.optional ~= nil then
        if value == nil then return nil end
        return check(shape.optional, value, active, path)
    end
    if shape.literal ~= nil then
        if value ~= shape.literal then return path .. ": unexpected literal value" end
        return nil
    end
    if shape.instance then
        if typeof(value) ~= "Instance" or not value:IsA(shape.instance) then return path .. ": expected " .. shape.instance end
        return nil
    end
    if shape.enum then
        if typeof(value) ~= "EnumItem" or tostring(value.EnumType) ~= "Enum." .. shape.enum then return path .. ": expected Enum." .. shape.enum end
        return nil
    end
    if shape.union then
        local firstError: string? = nil
        for _, branch in shape.union do
            local message = check(branch, value, active, path)
            if message == nil then return nil end
            firstError = firstError or message
        end
        return path .. ": no union branch matched (" .. (firstError or "empty union") .. ")"
    end
    if typeof(value) ~= "table" then return path .. ": expected table" end
    if getmetatable(value) ~= nil then return path .. ": metatables are not supported" end
    if active[value] then return path .. ": cyclic table" end
    active[value] = true
    local message: string? = nil
    if shape.array then
        local n = #value
        for key in value do
            if typeof(key) ~= "number" or key % 1 ~= 0 or key < 1 or key > n then
                message = fieldPath(path, key) .. ": expected a dense array"; break
            end
        end
        if message == nil then
            for i = 1, n do
                if value[i] == nil then message = fieldPath(path, i) .. ": array hole"; break end
                message = check(shape.array, value[i], active, fieldPath(path, i))
                if message then break end
            end
        end
    elseif shape.dictionary then
        for key, item in value do
            if typeof(key) ~= "string" then message = path .. ": expected string keys"; break end
            message = check(shape.dictionary, item, active, fieldPath(path, key))
            if message then break end
        end
    else
        for key in value do
            if shape.record[key] == nil then message = fieldPath(path, key) .. ": unknown field"; break end
        end
        if message == nil then
            for name, field in shape.record do
                message = check(field, value[name], active, fieldPath(path, name))
                if message then break end
            end
        end
    end
    active[value] = nil
    return message
end

-- nil means success. Validate every declared position, even after an omitted value.
return function(parameters: { Parameter }, ...: any): string?
    if select("#", ...) > #parameters then return "arguments: too many arguments" end
    local active = {}
    for i, parameter in parameters do
        local message = check(parameter.shape, select(i, ...), active, parameter.name)
        if message then return message end
    end
    return nil
end
