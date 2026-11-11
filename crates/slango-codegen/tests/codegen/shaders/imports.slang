import common;

RWStructuredBuffer<float> data;

[shader("compute")]
[numthreads(32, 1, 1)]
void doubleValues(uint3 id : SV_DispatchThreadID)
{
    data[id.x] = twice(data[id.x]);
}
