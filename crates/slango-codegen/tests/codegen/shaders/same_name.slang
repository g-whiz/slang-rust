namespace first
{
struct Item
{
    float weight;
};
}

namespace second
{
struct Item
{
    uint count;
};
}

StructuredBuffer<first::Item> weights;
RWStructuredBuffer<second::Item> counts;

[shader("compute")]
[numthreads(64, 1, 1)]
void tally(uint3 id : SV_DispatchThreadID)
{
    counts[id.x].count = uint(weights[id.x].weight);
}
