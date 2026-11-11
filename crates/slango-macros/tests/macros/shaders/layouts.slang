struct Tail
{
    float3 direction;
};

struct Gap
{
    float weight;
    float4 colour;
};

struct Grid
{
    int cells[2][3];
    uint4 mask;
};

struct Inner
{
    float2 offset;
    uint id;
};

struct Nested
{
    float3 position;
    Inner inner;
    Gap gap;
    float weight;
};

struct Wide
{
    float a;
    float b;
    float c;
    float d;
    uint e;
    uint f;
    int g;
    int h;
};

StructuredBuffer<Tail> tails;
StructuredBuffer<Gap> gaps;
StructuredBuffer<Grid> grids;
RWStructuredBuffer<Nested> nested;
StructuredBuffer<Wide> wides;

[shader("compute")]
[numthreads(64, 1, 1)]
void combine(uint3 id : SV_DispatchThreadID)
{
    nested[id.x].weight = tails[id.x].direction.x + gaps[id.x].weight + float(grids[id.x].cells[1][2]) + wides[id.x].d;
}
