RWStructuredBuffer<float3> points;

[shader("compute")]
[numthreads(64, 1, 1)]
void move(uint3 id : SV_DispatchThreadID)
{
    points[id.x] += float3(1.0, 0.0, 0.0);
}
