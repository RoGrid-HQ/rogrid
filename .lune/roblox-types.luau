-- Refresh the compile-time class/enum names from the pinned Lune reflection database.
local fs = require("@lune/fs")
local roblox = require("@lune/roblox")
local db = roblox.getReflectionDatabase()
local classes = {}
for _, name in db:GetClassNames() do
    local current = name
    while current do
        if current == "Instance" then table.insert(classes, name); break end
        current = db:GetClass(current).Superclass
    end
end
-- Object-derived values such as EditableMesh are not Instance references.
for kind, names in { classes = classes, enums = db:GetEnumNames() } do
    table.sort(names)
    fs.writeFile(`crates/rogrid/src/codegen/{kind}.txt`, table.concat(names, "\n") .. "\n")
end
