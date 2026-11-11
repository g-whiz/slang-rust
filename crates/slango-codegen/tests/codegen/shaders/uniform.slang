struct Light
{
    float3 colour;
    float intensity;
};

struct Params
{
    float scale;
    Light light;
    bool enabled;
    float2x2 rotation;
    int weights[3];
};

uniform Params params;
uniform Light extra;
StructuredBuffer<Light> lights;
RWStructuredBuffer<float4> output;

[shader("compute")]
[numthreads(64, 1, 1)]
void shade(uint3 id : SV_DispatchThreadID)
{
    float2 turned = mul(params.rotation, float2(params.scale, float(params.weights[id.x % 3])));
    float3 colour = lights[id.x].colour * lights[id.x].intensity + extra.colour * extra.intensity;
    if (params.enabled)
    {
        colour += params.light.colour * params.light.intensity;
    }
    output[id.x] = float4(colour, turned.x);
}
