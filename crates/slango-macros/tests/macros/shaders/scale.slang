RWStructuredBuffer<float> data;
uniform float factor;

[shader("compute")]
[numthreads(16, 2, 1)]
void scale(uint3 id : SV_DispatchThreadID)
{
    if (id.y == 0) { data[id.x] *= factor; }
}
