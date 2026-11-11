struct Motion
{
    float2 velocity;
    float drag;
};

struct Body
{
    float3 position;
    Motion motion;
    uint id;
};

StructuredBuffer<Motion> motions;
RWStructuredBuffer<Body> bodies;

[shader("compute")]
[numthreads(64, 1, 1)]
void push(uint3 id : SV_DispatchThreadID)
{
    bodies[id.x].motion = motions[id.x];
}

[shader("compute")]
[numthreads(64, 1, 1)]
void settle(uint3 id : SV_DispatchThreadID)
{
    bodies[id.x].motion.velocity *= 1.0 - bodies[id.x].motion.drag;
}
