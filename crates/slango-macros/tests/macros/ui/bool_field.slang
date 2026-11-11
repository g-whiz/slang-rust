struct Cell
{
    float weight;
    bool alive;
};

RWStructuredBuffer<Cell> cells;

[shader("compute")]
[numthreads(64, 1, 1)]
void grow(uint3 id : SV_DispatchThreadID)
{
    if (cells[id.x].alive)
    {
        cells[id.x].weight *= 2.0;
    }
}
