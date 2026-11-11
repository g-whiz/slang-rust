struct Settings
{
    float4 tint;
    float2x2 rotation;
    uint mode;
};

ConstantBuffer<Settings> settings;
Texture2D<float4> image;
SamplerState linearSampler;
StructuredBuffer<int4> offsets;
RWStructuredBuffer<float4> output;
uniform int weights[3];
uniform bool enabled;

[shader("compute")]
[numthreads(8, 8, 1)]
void shadeImage(uint3 id : SV_DispatchThreadID)
{
    float2 uv = float2(id.xy) / 8.0;
    float4 colour = image.SampleLevel(linearSampler, mul(settings.rotation, uv), 0) * settings.tint;
    if (enabled && settings.mode > 0) { colour += float4(offsets[id.x] * weights[id.x % 3]); }
    if (id.x < 8 && id.y < 8) { output[id.y * 8 + id.x] = colour; }
}

[shader("compute")]
[numthreads(64, 1, 1)]
void clear_output(uint3 id : SV_DispatchThreadID)
{
    output[id.x] = float4(0.0);
}
