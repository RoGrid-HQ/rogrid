-- Shared native values for Lune validation and Studio transport tests.
return function(r)
    local V2, V3, C = r.Vector2, r.Vector3, r.Color3
    local values = {
        string = "hello", number = 42, boolean = false, buffer = buffer.fromstring("bytes"),
        Vector2 = V2.new(1,2), Vector3 = V3.new(1,2,3), Vector2int16 = r.Vector2int16.new(1,2),
        Vector3int16 = r.Vector3int16.new(1,2,3), CFrame = r.CFrame.new(1,2,3),
        Color3 = C.new(0.1,0.2,0.3), BrickColor = r.BrickColor.new("White"),
        UDim = r.UDim.new(0.5,1), UDim2 = r.UDim2.new(0.5,1,0.5,2),
        Rect = r.Rect.new(V2.new(0,0),V2.new(1,1)), Ray = r.Ray.new(V3.zero,V3.one),
        Region3 = r.Region3.new(V3.zero,V3.one),
        Region3int16 = r.Region3int16.new(r.Vector3int16.new(),r.Vector3int16.new(1,1,1)),
        NumberRange = r.NumberRange.new(1,2), NumberSequence = r.NumberSequence.new(1),
        NumberSequenceKeypoint = r.NumberSequenceKeypoint.new(0,1), ColorSequence = r.ColorSequence.new(C.new()),
        ColorSequenceKeypoint = r.ColorSequenceKeypoint.new(0,C.new()), Axes = r.Axes.new(), Faces = r.Faces.new(),
        PhysicalProperties = r.PhysicalProperties.new(1,0.5,0.5,1,1),
        Font = r.Font.new("rbxasset://fonts/families/SourceSansPro.json"), EnumItem = r.Enum.Material.Plastic,
    }
    -- Lune does not implement DateTime. Studio covers this engine-only value.
    if r.DateTime then values.DateTime = r.DateTime.fromUnixTimestampMillis(1000) end
    return values
end
