struct Narrow
{
    float3 a;
};

RWStructuredBuffer<Narrow> items;

[shader("compute")]
[numthreads(1, 1, 1)]
void step()
{
    items[0].a.x = 7.0;
}
