

// List of status currently affecting an entity
// The status can be things like rooted, stunned, silenced ...
// 
// NOTE: We will probably need to keep a refcount of effects 
// producing these status, to know when to remove it
struct Status;
