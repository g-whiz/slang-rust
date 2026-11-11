uniform float time;
uniform float brightness;
uniform float scale;

struct VOut
{
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
};

[shader("vertex")]
VOut vsMain(uint vertex : SV_VulkanVertexID)
{
    float2 corner = float2(float((vertex << 1) & 2), float(vertex & 2));
    VOut output;
    output.position = float4(corner * 2.0 - 1.0, 0.0, 1.0);
    output.uv = float2(corner.x, 1.0 - corner.y);
    return output;
}

[shader("fragment")]
float4 fsMain(VOut input) : SV_Target
{
    float2 p = (input.uv * 2.0 - 1.0) * scale;
    float3 colour = 0.5 + 0.5 * cos(time + float3(p.x, p.y, p.x) + float3(0.0, 2.0, 4.0));
    return float4(colour * brightness, 1.0);
}
