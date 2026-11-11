RWStructuredBuffer<float> data;

[shader("compute")]
[numthreads(64, 1, 1)]
void broken(uint3 id : SV_DispatchThreadID)
{
    data[id.x] = undefinedValue;
}
