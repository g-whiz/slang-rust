struct VIn { float2 position : POSITION; float2 uv : TEXCOORD0; };
struct VOut { float4 position : SV_Position; float2 uv : TEXCOORD0; };

uniform float4x4 transform;
Texture2D<float4> image;
SamplerState imageSampler;

[shader("vertex")]
VOut vsMain(VIn input)
{
    VOut output;
    output.position = mul(transform, float4(input.position, 0.0, 1.0));
    output.uv = input.uv;
    return output;
}

[shader("fragment")]
float4 fsMain(VOut input) : SV_Target
{
    return image.Sample(imageSampler, input.uv);
}
