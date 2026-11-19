struct Item
{
    int3 a;
    float b;
};

RWStructuredBuffer<Item> items;

[shader("compute")]
[numthreads(1, 1, 1)]
void step()
{
    items[0].b = float(items[0].a.x) + 1.0;
}
