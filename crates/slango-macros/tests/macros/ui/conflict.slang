RWStructuredBuffer<float> data;

[shader("compute")]
[numthreads(64, 1, 1)]
void scale_twice(uint3 id : SV_DispatchThreadID)
{
    data[id.x] *= 4.0;
}

[shader("compute")]
[numthreads(64, 1, 1)]
void scaleTwice(uint3 id : SV_DispatchThreadID)
{
    data[id.x] *= 2.0;
}
