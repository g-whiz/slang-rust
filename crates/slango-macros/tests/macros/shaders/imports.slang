import my_helpers;

RWStructuredBuffer<float> data;
uniform uint count;

[shader("compute")]
[numthreads(32, 1, 1)]
void grow(uint3 id : SV_DispatchThreadID)
{
    if (id.x < count) { data[id.x] = quadruple(data[id.x]) + 1.0; }
}
