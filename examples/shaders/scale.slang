struct Params
{
    float scale;
    uint count;
};

RWStructuredBuffer<float> data;
ConstantBuffer<Params> params;

[shader("compute")]
[numthreads(64, 1, 1)]
void scale(uint3 id : SV_DispatchThreadID, uniform float bias)
{
    if (id.x < params.count)
        data[id.x] = data[id.x] * params.scale + bias;
}

[shader("compute")]
[numthreads(64, 1, 1)]
void clear(uint3 id : SV_DispatchThreadID)
{
    if (id.x < params.count)
        data[id.x] = 0.0;
}
