StructuredBuffer<float> x;
StructuredBuffer<float> y;
RWStructuredBuffer<float> result;
uniform float a;
uniform uint count;

[shader("compute")]
[numthreads(64, 1, 1)]
void saxpy(uint3 id : SV_DispatchThreadID)
{
    if (id.x < count) { result[id.x] = a * x[id.x] + y[id.x]; }
}
