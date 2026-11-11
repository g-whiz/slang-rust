import helpers;

RWStructuredBuffer<float> data;
uniform float factor;
uniform uint count;

[shader("compute")]
[numthreads(64, 1, 1)]
void scale(uint3 id : SV_DispatchThreadID)
{
    if (id.x < count) { data[id.x] *= factor; }
}

[shader("compute")]
[numthreads(8, 4, 2)]
void double_each(uint3 id : SV_DispatchThreadID)
{
    if (id.x < count) { data[id.x] = twice(data[id.x]); }
}
