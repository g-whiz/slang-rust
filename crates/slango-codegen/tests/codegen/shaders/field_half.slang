struct Cell
{
    float weight;
    half extra;
};

RWStructuredBuffer<Cell> cells;

[shader("compute")]
[numthreads(64, 1, 1)]
void grow(uint3 id : SV_DispatchThreadID)
{
    cells[id.x].weight *= 2.0;
}
