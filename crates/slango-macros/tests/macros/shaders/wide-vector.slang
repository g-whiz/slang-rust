struct Wide
{
    float4 a;
};

RWStructuredBuffer<Wide> items;

[shader("compute")]
[numthreads(1, 1, 1)]
void step()
{
    items[0].a.w = 7.0;
}
