#include <framework/mlt.h>

mlt_producer producer_anicode_init(mlt_profile profile, mlt_service_type type, const char *id, char *arg);

MLT_REPOSITORY
{
    MLT_REGISTER(mlt_service_producer_type, "anicode", producer_anicode_init);
}
