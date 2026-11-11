RWStructuredBuffer<float> data;
uniform double factor;

[shader("compute")]
[numthreads(64, 1, 1)]
void scale(uint3 id : SV_DispatchThreadID)
{
    data[id.x] = float(data[id.x] * factor);
}
