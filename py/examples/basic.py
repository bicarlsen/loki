# %%
import polars as pl
import loki

# %%
df = loki.get_df(interactive="btrJOA")

# %%
sdf = (
    df
    # .group_by("segment").mean()
    .select(["cafmBias", "cafmCurrent"])
)
# %%
loki.output(sdf)
# %%
