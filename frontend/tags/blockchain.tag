<blockchain>
    <table class="table table-striped">
        <thead>
        <tr>
            <th>Date</th>
            <th>Height</th>
        </tr>
        </thead>
        <tbody>
        <tr each={ blocks }>
            <td>{ propose_time }</td>
            <td><a href="/#blockchain/block/{ height }">{ height }</a></td>
        </tr>
        </tbody>
    </table>

    <nav>
        <ul class="pager">
            <li class="previous"><a href="#" onclick={ previous }><span aria-hidden="true">&larr;</span> Older</a></li>
            <li class="next"><a href="#" onclick={ next }>Newer <span aria-hidden="true">&rarr;</span></a></a></li>
        </ul>
    </nav>

    <a class="btn btn-lg btn-block btn-default" href="/#">Back</a>

    <script>
        var self = this;

        this.title = 'Blockchain';

        // TODO refactor ajax requests

        previous(e) {
            e.preventDefault();
            $.ajax({
                method: 'GET',
                url: this.api.baseUrl + '/blockchain/blocks?count=10&from=' + (self.blocks[0].height - 9),
                success: function(data, textStatus, jqXHR) {
                    self.blocks = data;
                    self.update();
                },
                error: function(jqXHR, textStatus, errorThrown) {
                    console.error(textStatus);
                }
            });
        }

        next(e) {
            e.preventDefault();
            $.ajax({
                method: 'GET',
                url: this.api.baseUrl + '/blockchain/blocks?count=10&from=' + (self.blocks[0].height + 11),
                success: function(data, textStatus, jqXHR) {
                    self.blocks = data;
                    self.update();
                },
                error: function(jqXHR, textStatus, errorThrown) {
                    console.error(textStatus);
                }
            });
        }

        $.ajax({
            method: 'GET',
            url: this.api.baseUrl + '/blockchain/blocks?count=10',
            success: function(data, textStatus, jqXHR) {
                self.blocks = data;
                self.update();
            },
            error: function(jqXHR, textStatus, errorThrown) {
                console.error(textStatus);
            }
        });
    </script>
</blockchain>